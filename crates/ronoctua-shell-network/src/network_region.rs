//! THE NETWORK VIEW IN THE FOOTER'S REGION (`LEADER s n n`) — and the one
//! errand handler both its doors carry out ([`carry_out`]).
//!
//! # Why the errands are here, and handled once
//!
//! Every order of this view re-reads, and says what happened: the bus can
//! refuse — polkit, a daemon on its way down — and a screen that said "radio
//! on" when it did not turn on would be a door lying about the hardware. That
//! was written twice, the footer's `network_errand` and the window's `apply`,
//! over two copies of the same thirty arms. It is written once now, over the
//! door's [`Doorway`], and each door lends what it holds ([`Held`]) and the
//! lines to its threads ([`Lines`]).
//!
//! # What the footer keeps, and what this region keeps
//!
//! The footer reads NetworkManager and the two tunnels for the whole session
//! — the modeline's marks are drawn from them with no view open — and tells
//! this region every reading, and the line to the tunnels' thread, as it
//! hears them. What the VIEW reads with nobody else interested — the
//! telephone to the daemon that scans and joins, `/proc`'s sockets and
//! counters, the sweep's and the names' answers — is this crate's, kept for
//! the life of the process ([`Kept`]): the footer kept it in fields that
//! outlived the view, so a sweep asked with the view closed (`LEADER s n l`)
//! is on its rows when it opens — or, still running then, as soon as it comes
//! back: an answer wakes whichever opening is on screen when it comes, not
//! the door that asked — and a rate is never a difference over the hour the
//! view was closed.
//!
//! # It works with the view closed
//!
//! `LEADER s n s`, `s n r`, `s n d`, `s n i`, `s n l` and `s n a` are keys of
//! the leader's tree, and none of them needs a list on screen: a scratch view
//! answers for them ([`closed`]). The ones that DO need a row find nothing
//! under the cursor of a view nobody opened, and do nothing.

use std::cell::RefCell;

use crate::echo::Echo;
use crate::line::Line;
use crate::network::{
    Errand, Facts, Heard, Lan, Password, Press, Question, Shelf, Switch, Target, View, Watch, Wire,
};
use crate::region::{Closed, Doorway, Entry, Seen, Taken};
use ronoctua_network::Snapshot;

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "network";

/// The view as the binary registers it with the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What the view's errands act on, in either door — each door's own, lent
/// for one errand.
pub struct Held<'a> {
    /// The view.
    pub view: &'a mut View,
    /// The door's telephone to the daemon, and the access points it has seen.
    pub wire: &'a mut Wire,
    /// The last reading — read again after every order.
    pub net: &'a mut Snapshot,
    /// What `/proc` says, and what the view's threads found.
    pub watch: &'a mut Watch,
    /// What the two tunnels say.
    pub tunnels: &'a ronoctua_tunnel::Scene,
    /// The network being joined, followed until NetworkManager says how it
    /// went ([`follow_join`]) — and, joined with a typed password, the
    /// password, held for the span of one question ("keep it?") and TAKEN on
    /// either answer: a password left behind would be a password in memory
    /// with no owner and no deadline.
    pub join: &'a mut Option<Join>,
}

/// A network being joined, as a door follows it.
#[derive(Debug)]
pub struct Join {
    /// Its name.
    pub ssid: String,
    /// The password typed for it — `None` for a saved or open network, which
    /// has nothing to keep.
    pub password: Option<Password>,
    /// What NetworkManager started, to ask how it went.
    pub joining: ronoctua_network::Joining,
    /// Whether it came up: the question "keep it?" stands from then on.
    pub up: bool,
}

/// The way a thread of the view's brings its answer back to the door's loop.
pub type Back = Box<dyn FnOnce(Heard) + Send>;

/// The door's lines to the threads the view's errands start or order.
pub struct Lines<'a> {
    /// A way back for one thread of the view's slow work — a sweep, the
    /// names, a measurement — or `None` when the door has no loop to bring
    /// it to (a photograph's): the work is then said to be unavailable.
    pub lan: Option<&'a dyn Fn() -> Back>,
    /// The tunnels' thread, which carries out their orders.
    pub tunnels: Option<&'a ronoctua_tunnel::Hand>,
    /// What the door does when the radio is asked to look again: follow the
    /// END of the scan, so the list reads again then ([`crate::network::scans`]).
    /// `None` for a door that follows them for its whole life already.
    pub scans: Option<&'a dyn Fn()>,
}

/// The rows' facts, out of what a door lent.
fn facts_of<'b>(held: &'b Held<'_>) -> Facts<'b> {
    Facts {
        net: held.net,
        seen: held.wire.seen(),
        watch: held.watch,
        tunnels: held.tunnels,
    }
}

/// The name of one connection that is up — for the sentence about it — or
/// its path when the daemon has forgotten it between the key and the
/// sentence.
fn name_of_active(net: &Snapshot, at: &str) -> String {
    net.active
        .iter()
        .find(|a| a.at == at)
        .map_or_else(|| at.to_owned(), |a| a.id.clone())
}

/// The name of one stored profile — for the sentence about it.
fn name_of_saved(net: &Snapshot, at: &str) -> String {
    net.saved
        .iter()
        .find(|s| s.at == at)
        .map_or_else(|| at.to_owned(), |s| s.id.clone())
}

/// A registered location's readable label, out of the profile at `at` — the
/// country and city, or the raw id when the profile is not one of ours.
fn vpn_place(net: &Snapshot, at: &str) -> String {
    let id = name_of_saved(net, at);
    ronoctua_network::vpn::location_of(&id).map_or(id, |l| l.label())
}

/// The sentence for a profile that has just been removed.
///
/// ONE KEY, ONE IDEA. From the person's side, `f` on a Wi-Fi row and `f` on
/// the same network in the remembered shelf are the same act — and they were
/// two sentences, "forgotten" and "deleted", because the first went through
/// `Errand::Forget` and the second through `Errand::Delete`. What tells them
/// apart is the profile's KIND, which the snapshot already carries.
fn removed_saying(net: &Snapshot, at: &str) -> &'static str {
    let wireless = net
        .saved
        .iter()
        .find(|s| s.at == at)
        .is_some_and(|s| s.kind == crate::network::WIRELESS);
    if wireless {
        crate::key::ECHO_NETWORK_FORGOTTEN
    } else {
        crate::key::ECHO_NETWORK_DELETED
    }
}

/// Carries out what the view asked for, in whichever door it is drawn — the
/// footer's region or the window ([`crate::network_window`]).
///
/// EVERY ORDER RE-READS: the watch thread would bring the same reading a
/// moment later, and this is what makes the screen answer the key that was
/// just pressed rather than a quarter of a second afterwards.
pub fn carry_out(errand: Errand, held: &mut Held<'_>, lines: &Lines<'_>, way: &mut dyn Doorway) {
    use crate::key;
    let t = |way: &dyn Doorway, k: &'static str| way.texts().t(k);
    let said: Option<(String, Echo)> = match errand {
        Errand::Nothing => None,
        // AN INFORMATIVE ROW COPIES ITSELF — see `chrome::information_of`,
        // and `chrome::copied` for the one place the six doors share.
        Errand::Copy(text) => Some((crate::chrome::copied(&text, way.texts()), Echo::Brief)),
        // A PASSWORD WAITING FOR "KEEP IT?" goes with the view that asked it.
        Errand::Close => {
            *held.join = None;
            way.close();
            return;
        }
        // THE SWEEP GOES ON A THREAD, and takes a second: it sends 254
        // packets and waits for the slowest answer, which is a second a
        // door's one loop must not spend — the `systemd-run --scope` trap.
        Errand::Discover => {
            held.watch.read_vendors();
            let address = crate::network::carrying(held.net)
                .and_then(|l| l.address.clone())
                .or_else(|| held.net.links.iter().find_map(|l| l.address.clone()));
            match (address, lines.lan) {
                (Some(address), Some(back)) => {
                    held.watch.sweeping(true);
                    if !crate::network::spawn(Lan::Sweep(address), back()) {
                        held.watch.sweeping(false);
                    }
                    None
                }
                // NO ADDRESS IS NO NETWORK TO SWEEP, and saying so beats a
                // key that looks broken.
                _ => Some((t(way, key::ECHO_NETWORK_NO_LINK), Echo::Normal)),
            }
        }
        // A LINE, ON THE DOOR'S OWN LINE: the password hidden, a path and a
        // Tor value in the clear. The answer comes back to `View::answered`
        // under the question's id, in either door.
        Errand::Ask(question) => {
            way.ask(Line::new(question.id, question.secret, None));
            None
        }
        // NEITHER KIND IS A REFUSAL WITH A REASON — see
        // `network::read_tunnel`.
        Errand::NotATunnel(name) => Some((
            way.texts()
                .t_with(key::ECHO_NETWORK_NOT_A_TUNNEL, &[("file", &name)]),
            Echo::Normal,
        )),
        Errand::Said(said) => Some((said, Echo::Normal)),
        // THE CLIPBOARD, off the loop, for the file picker (`Region::pasted`,
        // `Content::pasted`).
        Errand::Paste => {
            way.paste();
            None
        }
        Errand::Bring(path, kind) => {
            let name = ronoctua_network::vpn::name_of(&path);
            let imported = held.wire.import(&path, kind);
            let said = match &imported {
                Ok(()) => way
                    .texts()
                    .t_with(key::ECHO_NETWORK_IMPORTED, &[("network", &name)]),
                Err(e) => way.texts().t_with(
                    key::ECHO_NETWORK_IMPORT_FAILED,
                    &[
                        ("file", &name),
                        ("reason", &crate::network_refusal::reason(e, way.texts())),
                    ],
                ),
            };
            way.say(said, Echo::Normal);
            // A VPN THAT ASKS FOR A PASSWORD asks for it NOW, on the door's
            // line — the view decides (`after_bring`).
            if let Errand::Ask(question) = held.view.after_bring(imported.is_ok()) {
                way.ask(Line::new(question.id, question.secret, None));
            }
            None
        }
        // THE VPN'S USER AND PASSWORD, stored in its profile: the password
        // goes to NetworkManager in this one call and leaves the door with
        // the errand.
        Errand::VpnSecret {
            tunnel,
            user,
            password,
        } => Some((
            match held.wire.store_vpn_secret(&tunnel, &user, &password) {
                Ok(()) => way
                    .texts()
                    .t_with(key::ECHO_NETWORK_VPN_STORED, &[("network", &tunnel)]),
                Err(e) => way.texts().t_with(
                    key::ECHO_NETWORK_VPN_STORE_FAILED,
                    &[("reason", &crate::network_refusal::reason(&e, way.texts()))],
                ),
            },
            Echo::Normal,
        )),
        // REGISTER A FOLDER OF VPN SERVERS: stage the key (`0600`) and the
        // public locations, then start the root install unit, which seals the
        // key in the TPM and installs the locations; the sync writes the
        // profiles after. The key leaves the door ONLY as the one `0600` staged
        // file. The install is a oneshot of a second or two, so `--wait` here is
        // the brief pause of an explicit action, not the minutes a build is.
        Errand::VpnRegister { folder, key } => Some((
            match ronoctua_network::vpn::stage_registration(
                std::path::Path::new(&folder),
                key.as_str(),
                std::path::Path::new(ronoctua_network::vpn::STAGING_DIR),
            ) {
                Err(why) => way.texts().t(match why {
                    ronoctua_network::vpn::RegisterWhy::NotAKey => key::ECHO_NETWORK_VPN_NOT_A_KEY,
                    ronoctua_network::vpn::RegisterWhy::NoServers => {
                        key::ECHO_NETWORK_VPN_NO_SERVERS
                    }
                    ronoctua_network::vpn::RegisterWhy::Unreadable => {
                        key::ECHO_NETWORK_VPN_UNREADABLE
                    }
                }),
                Ok(reg) => {
                    // The `systemctl` by its baked path where there is one; a
                    // session's PATH has it either way.
                    const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
                        Some(c) => c,
                        None => "systemctl",
                    };
                    // THE INSTALL seals the key, writes the locations AND waits
                    // for the sync (its `ExecStartPost`), so by the time this
                    // returns the profiles are on disk and NetworkManager has
                    // reloaded them — the view's re-read below then shows the
                    // list filled, instead of the person having to reopen it.
                    // Only the install unit is the person's to start (the polkit
                    // rule); the sync it waits on is root's.
                    let ok = std::process::Command::new(SYSTEMCTL)
                        .args(["start", "--wait", "ronoctua-vpn-install.service"])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if ok {
                        let count = reg.count.to_string();
                        way.texts()
                            .t_with(key::ECHO_NETWORK_VPN_REGISTERED, &[("count", &count)])
                    } else {
                        way.texts().t(key::ECHO_NETWORK_VPN_INSTALL_FAILED)
                    }
                }
            },
            Echo::Normal,
        )),
        // A RUNTIME SETTING CHANGED — IPv6, the kill switch or the DNS. No root
        // unit and no secret: stage the `key=value` file in the person's own
        // runtime folder and the path unit fires the sync, which persists it and
        // applies it (the kill switch's nftables table, the profiles' IPv6/DNS).
        // No `--wait`: the sync is a beat behind the key and the view already
        // reads the flipped switch.
        Errand::VpnSettings(settings) => Some((
            match ronoctua_network::vpn::stage_settings(
                &settings,
                std::path::Path::new(ronoctua_network::vpn::SETTINGS_STAGING),
            ) {
                Ok(()) => way.texts().t(key::ECHO_NETWORK_VPN_SETTINGS),
                Err(_) => way.texts().t(key::ECHO_NETWORK_VPN_SETTINGS_FAILED),
            },
            Echo::Normal,
        )),
        // BRING A LOCATION UP, THROUGH ROOT — the only agent NetworkManager
        // hands the sealed key to for one the person raised. The call returns
        // at once (root brings it up on a thread and takes it back down if the
        // server never answers), so the word here is "connecting" and the list
        // and the modeline tell how it went.
        Errand::VpnRaise(at) => {
            let place = vpn_place(held.net, &at);
            Some((
                match held.wire.raise_tunnel(&at) {
                    Ok(()) => way
                        .texts()
                        .t_with(key::ECHO_NETWORK_VPN_CONNECTING, &[("place", &place)]),
                    Err(e) => crate::network_words::refused(&e, way.texts()),
                },
                Echo::Normal,
            ))
        }
        // FORGET A LOCATION: stage its id for the sync, which drops it from the
        // locations and prunes its profile. The key is the whole set's, so it
        // stays. The path unit fires the sync; the list loses the row on the
        // next read.
        Errand::VpnForget(id) => {
            let place =
                ronoctua_network::vpn::location_of(&id).map_or_else(|| id.clone(), |l| l.label());
            Some((
                match ronoctua_network::vpn::stage_forget(
                    &id,
                    std::path::Path::new(ronoctua_network::vpn::FORGET_STAGING),
                ) {
                    Ok(()) => way
                        .texts()
                        .t_with(key::ECHO_NETWORK_VPN_FORGOTTEN, &[("place", &place)]),
                    Err(_) => way.texts().t(key::ECHO_NETWORK_VPN_FORGET_FAILED),
                },
                Echo::Normal,
            ))
        }
        // THE GATEWAY'S ROUND TRIP, on a thread: three echoes a fifth of a
        // second apart are the better part of a second.
        Errand::Measure(gateway) => match lines.lan {
            Some(back) => {
                held.watch.measuring(gateway);
                crate::network::spawn(Lan::Measure(gateway), back());
                None
            }
            None => Some((t(way, key::ECHO_NETWORK_UNAVAILABLE), Echo::Normal)),
        },
        // BECOMING THE NETWORK, from either door: NetworkManager keeps no
        // per-client state, so a second telephone is not a second truth. The
        // password is made here, shown on the row and kept where both doors
        // read it (`network::sharing`), born owner-only — and said for long
        // enough to be read.
        Errand::Hotspot(true) => {
            let name = crate::network::hostname();
            Some((
                match ronoctua_network::hotspot_password()
                    .and_then(|word| held.wire.hotspot(&name, &word).map(|()| word))
                {
                    Ok(word) => {
                        if let Err(e) =
                            crate::link_words::sharing::keep(&crate::link_words::sharing::Kept {
                                network: name.clone(),
                                password: word.clone(),
                            })
                        {
                            tracing::warn!(error = %e, "the hotspot's password was not kept for the other door");
                        }
                        held.view.sharing(Some(word.clone()));
                        way.texts().t_with(
                            key::ECHO_NETWORK_HOTSPOT,
                            &[("network", &name), ("password", &word)],
                        )
                    }
                    Err(e) => way.texts().t_with(
                        key::ECHO_NETWORK_HOTSPOT_FAILED,
                        &[("reason", &crate::network_refusal::reason(&e, way.texts()))],
                    ),
                },
                Echo::Long,
            ))
        }
        Errand::Hotspot(false) => {
            crate::link_words::sharing::forget();
            held.view.sharing(None);
            Some((
                t(
                    way,
                    if held
                        .wire
                        .disconnect(ronoctua_network::Medium::Radio)
                        .is_ok()
                    {
                        key::ECHO_NETWORK_DISCONNECTED
                    } else {
                        key::ECHO_NETWORK_UNAVAILABLE
                    },
                ),
                Echo::Normal,
            ))
        }
        Errand::Scan => {
            if let Some(follow) = lines.scans {
                follow();
            }
            held.wire.scan();
            None
        }
        Errand::Radio(on) => Some((
            t(
                way,
                match held.wire.radio(on) {
                    Ok(()) if on => key::ECHO_NETWORK_RADIO_ON,
                    Ok(()) => key::ECHO_NETWORK_RADIO_OFF,
                    Err(_) => key::ECHO_NETWORK_UNAVAILABLE,
                },
            ),
            Echo::Normal,
        )),
        Errand::Airplane(on) => match held.wire.airplane(on) {
            Ok(()) => {
                tracing::info!(airplane = on, "aeroplane mode");
                None
            }
            Err(e) => Some((crate::network_words::refused(&e, way.texts()), Echo::Normal)),
        },
        Errand::Disconnect(medium, interface) => Some((
            t(
                way,
                if held.wire.disconnect_link(medium, &interface).is_ok() {
                    key::ECHO_NETWORK_DISCONNECTED
                } else {
                    key::ECHO_NETWORK_UNAVAILABLE
                },
            ),
            Echo::Normal,
        )),
        // THE LINK'S FACTS, on the message line: band, rate and signal. It is
        // the answer to "why is this slow?" without leaving the screen.
        Errand::Facts => Some((
            match crate::network::carrying(held.net) {
                // A CABLE AND A PHONE have no band and no signal to say.
                Some(l) if l.medium != ronoctua_network::Medium::Radio => way.texts().t_with(
                    key::ECHO_NETWORK_LINK_WIRED,
                    &[
                        ("network", &l.interface),
                        ("rate", &crate::network::rate_of(l, way.texts())),
                    ],
                ),
                Some(l) => way.texts().t_with(
                    key::ECHO_NETWORK_LINK,
                    &[
                        ("network", l.network.as_deref().unwrap_or(&l.interface)),
                        ("band", &crate::network::band_text(l.band, way.texts())),
                        ("rate", &crate::network::rate_of(l, way.texts())),
                        ("signal", &format!("{}%", l.signal)),
                    ],
                ),
                None => t(way, key::ECHO_NETWORK_NO_LINK),
            },
            Echo::Normal,
        )),
        Errand::Forget(ssid) => Some((
            if held.wire.forget(&ssid).is_ok() {
                way.texts()
                    .t_with(key::ECHO_NETWORK_FORGOTTEN, &[("network", &ssid)])
            } else {
                t(way, key::ECHO_NETWORK_UNAVAILABLE)
            },
            Echo::Normal,
        )),
        // JOINING, IN MEMORY, AND FOLLOWED. A typed password joins for this
        // boot, kept by NetworkManager itself (`Manager::join_for_this_boot`):
        // no agent has it yet, and NetworkManager does not take an
        // agent-owned one from the call. What NetworkManager says of it comes
        // with the next reading (`follow_join`), and only then does the door
        // say "connected" — and ask "keep it?", the answer that seals it
        // (`Manager::remember`). Storing first and deleting on a refusal
        // would leave the password on disk in the meantime.
        Errand::Connect(target, password) => {
            let typed = password.filter(|_| target.secured);
            let result = held
                .wire
                .connect(&target.ssid, typed.as_ref().map(Password::as_str));
            Some(match result {
                Ok(joining) => {
                    *held.join = Some(Join {
                        ssid: target.ssid.clone(),
                        password: typed,
                        joining,
                        up: false,
                    });
                    (
                        way.texts()
                            .t_with(key::ECHO_NETWORK_JOINING, &[("network", &target.ssid)]),
                        Echo::Normal,
                    )
                }
                Err(e) => (
                    way.texts().t_with(
                        key::ECHO_NETWORK_FAILED,
                        &[
                            ("network", &target.ssid),
                            ("reason", &crate::network_refusal::reason(&e, way.texts())),
                        ],
                    ),
                    Echo::Normal,
                ),
            })
        }
        // THE ANSWER TO "KEEP IT?". The join is TAKEN before anything can
        // fail, on both answers: a password left behind after an error would
        // be a password in memory with no owner and no deadline. `yes` seals
        // it and writes the profile to disk; the link that is up stays up.
        Errand::Remember(ssid, yes) => {
            let kept = held.join.take();
            Some((
                if yes {
                    match kept.filter(|j| j.ssid == ssid && j.up) {
                        Some(Join {
                            password: Some(password),
                            joining,
                            ..
                        }) => match held.wire.remember(&joining, &ssid, password.as_str()) {
                            Ok(()) => way
                                .texts()
                                .t_with(key::ECHO_NETWORK_STORED, &[("network", &ssid)]),
                            Err(e) => way.texts().t_with(
                                key::ECHO_NETWORK_STORE_FAILED,
                                &[("reason", &crate::network_refusal::reason(&e, way.texts()))],
                            ),
                        },
                        _ => t(way, key::ECHO_NETWORK_UNAVAILABLE),
                    }
                } else {
                    t(way, key::ECHO_NETWORK_NO_STORED)
                },
                Echo::Normal,
            ))
        }
        Errand::Drop(at) => {
            let name = name_of_active(held.net, &at);
            Some((
                match held.wire.drop_it(&at) {
                    Ok(()) => way
                        .texts()
                        .t_with(key::ECHO_NETWORK_TUNNEL_DOWN, &[("network", &name)]),
                    Err(e) => crate::network_words::refused(&e, way.texts()),
                },
                Echo::Normal,
            ))
        }
        Errand::Raise(at) => {
            let name = name_of_saved(held.net, &at);
            Some((
                match held.wire.raise(&at) {
                    Ok(()) => way
                        .texts()
                        .t_with(key::ECHO_NETWORK_TUNNEL_UP, &[("network", &name)]),
                    Err(e) => crate::network_words::refused(&e, way.texts()),
                },
                Echo::Normal,
            ))
        }
        Errand::Delete(at) => {
            let name = name_of_saved(held.net, &at);
            Some((
                match held.wire.delete(&at) {
                    Ok(()) => way
                        .texts()
                        .t_with(removed_saying(held.net, &at), &[("network", &name)]),
                    Err(e) => crate::network_words::refused(&e, way.texts()),
                },
                Echo::Normal,
            ))
        }
        // THE TUNNEL ORDERS GO TO THEIR OWN THREAD, which is where the
        // sockets live: a `PATCH` to Tailscale and a `systemctl start` both
        // take longer than a frame. Neither daemon keeps per-client state, so
        // whichever door's thread it is, it is not a second truth.
        Errand::Tunnel(order) => match lines.tunnels {
            Some(h) => {
                h.send(order);
                None
            }
            None => Some((t(way, key::ECHO_NETWORK_UNAVAILABLE), Echo::Normal)),
        },
        // TOR'S SETTINGS, and the sentence depends on whether there is a
        // daemon to tell. A fragment written while Tor is stopped is written
        // and not reloaded — which is the ordinary way to configure bridges
        // before the first start, and saying "written" as if something had
        // been applied would be a small lie on the one screen that exists to
        // stop them.
        Errand::TorConfigure(configured) => {
            let running = matches!(
                held.tunnels.standing,
                ronoctua_tunnel::tor::Standing::Running
            );
            match lines.tunnels {
                Some(h) => {
                    h.send(ronoctua_tunnel::Order::Configure(configured));
                    Some((
                        t(
                            way,
                            if running {
                                key::ECHO_TOR_WRITTEN
                            } else {
                                key::ECHO_TOR_NOT_RUNNING
                            },
                        ),
                        Echo::Normal,
                    ))
                }
                None => Some((t(way, key::ECHO_NETWORK_UNAVAILABLE), Echo::Normal)),
            }
        }
        Errand::TorAutostart(on) => match lines.tunnels {
            Some(h) => {
                h.send(ronoctua_tunnel::Order::Autostart(on));
                None
            }
            None => Some((t(way, key::ECHO_NETWORK_UNAVAILABLE), Echo::Normal)),
        },
    };
    *held.net = held.wire.look();
    if let Some((text, class)) = said {
        way.say(text, class);
    }
    // THE VIEW FOLLOWS THE NEW ROWS: a cable plugged in brings six facts, a
    // tunnel coming up brings a row, and a cursor past the end focuses
    // nothing. An ARRIVAL does not move the page; only the person's gesture
    // that asked this does, in its door.
    let rows = held.view.rows(&facts_of(held), way.columns(), way.texts());
    held.view.settle(&rows);
    way.repaint();
}

/// Follows the network being joined, on a reading of the daemon — in
/// whichever door holds it: NetworkManager's verdict is said when it comes,
/// and not when the join was asked.
///
/// - UP: "connected", and, joined with a typed password, the question "keep
///   it?" — never before: sealing a password the network refused stores
///   rubbish the agent hands over at every boot.
/// - REFUSED (the key) or FAILED (anything else, once the join's tries are
///   over): why, and the profile the join made goes, so the daemon does not
///   try a refused key again on its own; a saved profile stays.
/// - COMING: nothing yet.
pub fn follow_join(held: &mut Held<'_>, way: &mut dyn Doorway) {
    use crate::key;
    use ronoctua_network::Joined;
    let Some((ssid, typed, verdict)) = held.join.as_ref().filter(|j| !j.up).map(|j| {
        (
            j.ssid.clone(),
            j.password.is_some(),
            held.wire.joining(&j.joining),
        )
    }) else {
        return;
    };
    match verdict {
        Joined::Coming => return,
        Joined::Up => {
            way.say(
                way.texts()
                    .t_with(key::ECHO_NETWORK_CONNECTED, &[("network", &ssid)]),
                Echo::Normal,
            );
            if typed {
                if let Some(j) = held.join.as_mut() {
                    j.up = true;
                }
                let facts = Facts {
                    net: held.net,
                    seen: held.wire.seen(),
                    watch: held.watch,
                    tunnels: held.tunnels,
                };
                held.view
                    .ask_remember(ssid, &facts, way.columns(), way.texts());
            } else {
                *held.join = None;
            }
        }
        Joined::Refused | Joined::Failed => {
            // THE KEY, or anything else: only the join's own verdict says the
            // typed one was wrong (`Joined::Refused` — NetworkManager asking
            // for a new password, or an SAE network not letting the radio in
            // twice). A network that timed out or went away, after the join's
            // tries, is not a wrong password.
            let refusal = if verdict == Joined::Refused {
                ronoctua_network::refusal::Refusal::NoSecrets
            } else {
                ronoctua_network::refusal::Refusal::Failed
            };
            let why = anyhow::Error::from(ronoctua_network::refusal::Named(refusal));
            way.say(
                way.texts().t_with(
                    key::ECHO_NETWORK_FAILED,
                    &[
                        ("network", &ssid),
                        ("reason", &crate::network_refusal::reason(&why, way.texts())),
                    ],
                ),
                Echo::Normal,
            );
            if let Some(j) = held.join.take() {
                let _ = held.wire.abandon(&j.joining);
            }
        }
    }
    let rows = held.view.rows(&facts_of(held), way.columns(), way.texts());
    held.view.settle(&rows);
    way.repaint();
}

/// The view's gestures by their command ids — what the leader's tree,
/// `LEADER :` and the manual reach them by. What they do is press a key in
/// the view, which is the only place that knows what the cursor is on.
fn press_of_command(id: &str) -> Option<Press> {
    Some(match id {
        // THE AEROPLANE IS A SWITCH: the press reads what is and writes the
        // other, so the mark's click, the leader's `s n a` and the view's
        // `a` are one path.
        "network-airplane" => Press::Airplane,
        "network-scan" => Press::Scan,
        "network-wifi-toggle" => Press::Radio,
        "network-disconnect" => Press::Disconnect,
        "network-link" => Press::Facts,
        // WHO ELSE IS ON THIS NETWORK — the sweep.
        "network-lan" => Press::Discover,
        "network-section-next" => Press::Section,
        "network-section-previous" => Press::SectionBack,
        "network-forget" => Press::Forget,
        "network-up" => Press::Up,
        "network-down" => Press::Down,
        // TOR'S SHEET: commands because `network::OF_THE_VIEW` teaches them,
        // and everything it teaches can be RUN.
        "network-tor-more" => Press::More,
        "network-tor-less" => Press::Less,
        "network-tor-back" => Press::Escape,
        _ => return None,
    })
}

/// The typed answers the view's questions come back under — and that
/// `LEADER :` reaches with an argument.
const ANSWERS: [&str; 6] = [
    "network-connect",
    "network-import",
    "tor-setting",
    "tor-lines",
    "network-vpn-user",
    "network-vpn-password",
];

/// The argument `network-open` takes from a modeline mark — the row the view
/// opens on, with its section's title at the top: a mark is a door onto the
/// thing it is about. `None` for a row no mark opens on.
#[must_use]
pub fn opening_on(target: &Target) -> Option<String> {
    use crate::link_words::{TAILNET_OPENING, TOR_OPENING, tunnel_opening};
    match target {
        Target::Tunnel(at) => Some(tunnel_opening(at)),
        Target::Tunnels(Switch::Tailnet, _) => Some(TAILNET_OPENING.to_owned()),
        Target::Tunnels(Switch::Tor, _) => Some(TOR_OPENING.to_owned()),
        _ => None,
    }
}

/// The row [`opening_on`] named.
fn opened_on(argument: &str) -> Option<Target> {
    use crate::link_words::{TAILNET_OPENING, TOR_OPENING, tunnel_opened};
    match argument {
        TAILNET_OPENING => Some(Target::Tunnels(Switch::Tailnet, false)),
        TOR_OPENING => Some(Target::Tunnels(Switch::Tor, false)),
        other => tunnel_opened(other).map(|at| Target::Tunnel(at.to_owned())),
    }
}

/// The shelf behind the number the footer knows it by.
///
/// EVERY SHELF THE VIEW DRAWS MUST BE HERE, or the footer has no number to
/// scroll it by and it sits stuck at its first five rows — which is exactly
/// what the registered locations did until `Locations` was added: a hundred
/// servers behind a window that would not move.
const SHELVES: [Shelf; 6] = [
    Shelf::Wifi,
    Shelf::Remembered,
    Shelf::Ports,
    Shelf::Neighbours,
    Shelf::Peers,
    Shelf::Locations,
];

/// WHAT THE VIEW KEEPS FOR THE LIFE OF THE PROCESS, with the view closed or
/// open — what the footer kept in fields that outlived the view: the
/// telephone to the daemon (made when first wanted: a machine whose owner
/// never opens the list pays no bus connection for it), `/proc`'s sockets and
/// counters with what the sweep and the names found, and where the view's
/// threads bring that back.
struct Kept {
    wire: Option<Wire>,
    watch: Watch,
    sent: std::sync::mpsc::Sender<Heard>,
    heard: std::sync::mpsc::Receiver<Heard>,
    /// The wake of the opening on screen NOW, which a thread's answer rings
    /// ([`Kept::way_back`]); `None` while the view is closed.
    listening: std::sync::Arc<std::sync::Mutex<Option<crate::wake::Wake>>>,
}

impl Kept {
    fn fresh() -> Self {
        let (sent, heard) = std::sync::mpsc::channel();
        Self {
            wire: None,
            watch: Watch::default(),
            sent,
            heard,
            listening: std::sync::Arc::default(),
        }
    }

    /// The way back for the view's threads: what one found goes into the
    /// process's channel, and the opening on screen WHEN IT COMES BACK is
    /// woken to take it (`Region::woken`) — not the door that asked.
    ///
    /// # Why not the wake of the door that asked
    ///
    /// Because that door can be gone by the time the answer comes. A sweep
    /// asked with the view closed (`LEADER s n l`) is asked through a doorway
    /// whose wake wakes nothing, and a name asked by one opening comes back
    /// after `Esc` and a second opening, whose wake is a new channel of the
    /// loop. Each answer sat in the channel unread for the whole opening: the
    /// neighbours' shelf said a sweep was running, an address said "asking",
    /// and no name was ever asked again (`Watch::names_wanted` waits for the
    /// batch that is out).
    fn way_back(&self) -> impl Fn() -> Back + use<> {
        let sent = self.sent.clone();
        let listening = std::sync::Arc::clone(&self.listening);
        move || -> Back {
            let sent = sent.clone();
            let listening = std::sync::Arc::clone(&listening);
            Box::new(move |h| {
                let _ = sent.send(h);
                // The lock failing means a door panicked holding it, and the
                // process is going: there is nobody to wake.
                let wake = listening.lock().ok().and_then(|l| l.clone());
                if let Some(w) = wake {
                    w.call();
                }
            })
        }
    }

    /// One way back ([`Kept::way_back`]).
    fn back(&self) -> Back {
        (self.way_back())()
    }

    /// The opening that hears what comes back from now on — `None` as it
    /// goes, so an answer that comes with the view closed waits in the
    /// channel for the next opening, which takes it as it opens.
    fn listen(&self, wake: Option<crate::wake::Wake>) {
        if let Ok(mut l) = self.listening.lock() {
            *l = wake;
        }
    }

    /// What the view's threads found while nobody drew it, taken.
    fn take_what_was_heard(&mut self) -> bool {
        let mut any = false;
        while let Ok(h) = self.heard.try_recv() {
            self.watch.heard(h);
            any = true;
        }
        any
    }
}

thread_local! {
    /// The door's loop is one thread, and this is that thread's.
    static KEPT: RefCell<Kept> = RefCell::new(Kept::fresh());
}

/// Runs `f` over what the process keeps.
fn with_kept<T>(f: impl FnOnce(&mut Kept) -> T) -> T {
    KEPT.with(|k| f(&mut k.borrow_mut()))
}

/// What a command of the view does while it is CLOSED.
///
/// The view's gestures by name are pressed in a SCRATCH VIEW with what the
/// process keeps — a scan, the radio, a disconnect, the link's facts, a sweep,
/// the aeroplane all act on the machine and say so; a gesture that needs a row
/// finds nothing under a fresh cursor and does nothing. An answer typed for a
/// view nobody opened answers nothing. The window opens; `network-open`,
/// Tor's sheet and a path handed in from outside (`network-import <path>`,
/// which shows the file's reading) open the region.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "network-open" | "network-tor-open" => Closed::Opens,
        "network-window" => {
            way.open_window(vec!["network".to_owned()]);
            Closed::Taken
        }
        // A PATH HANDED IN FROM OUTSIDE opens the view to show it; with no
        // path, the file's question is a press of a view nobody opened.
        "network-import" => match argument {
            Some(_) => Closed::Opens,
            None => {
                scratch(Press::Import, way);
                Closed::Taken
            }
        },
        // `RET` ON THE FOCUSED NETWORK, of a view nobody opened.
        "network-connect" => {
            if argument.is_none() {
                scratch(Press::Enter, way);
            }
            Closed::Taken
        }
        // AN ANSWER TYPED FOR A VIEW NOBODY OPENED answers nothing.
        answer if ANSWERS.contains(&answer) => Closed::Taken,
        other => match press_of_command(other) {
            Some(press) => {
                scratch(press, way);
                Closed::Taken
            }
            None => Closed::NotMine,
        },
    }
}

/// One press in a view nobody opened ([`closed`]).
///
/// A PHOTOGRAPH'S DOOR — the footer's camera, a test — reaches nothing of
/// the machine: with no telephone kept yet it presses against one with no
/// daemon behind it (`Wire::planted`), which refuses every order in the
/// words a machine with no NetworkManager hears, and keeps nothing.
fn scratch(press: Press, way: &mut dyn Doorway) {
    with_kept(|k| {
        // WHAT IT STARTS COMES BACK TO THE OPENING THERE IS THEN — none yet,
        // so it waits for the next one (`Kept::way_back`).
        let back = k.way_back();
        let mut aside = None;
        let wire = match (k.wire.as_mut(), way.photographing()) {
            (Some(w), _) => w,
            (None, true) => aside.insert(Wire::planted(Snapshot::default(), Vec::new())),
            (None, false) => k.wire.insert(Wire::fresh()),
        };
        let mut net = wire.look();
        let tunnels = ronoctua_tunnel::Scene::default();
        let mut view = View::fresh();
        let errand = view.key(
            press,
            &Facts {
                net: &net,
                seen: wire.seen(),
                watch: &k.watch,
                tunnels: &tunnels,
            },
            way.columns(),
            way.texts(),
        );
        let wake = way.wake();
        let follow = move || crate::network::scans::follow(wake.clone());
        let photographing = way.photographing();
        let lines = Lines {
            lan: (!photographing).then_some(&back as &dyn Fn() -> Back),
            tunnels: None,
            scans: (!photographing).then_some(&follow as &dyn Fn()),
        };
        let mut join = None;
        let mut held = Held {
            view: &mut view,
            wire,
            net: &mut net,
            watch: &mut k.watch,
            tunnels: &tunnels,
            join: &mut join,
        };
        carry_out(errand, &mut held, &lines, way);
    });
}

/// Opens the view — the footer asked whether the region is free first — on
/// Tor's sheet when that is what was asked: a LEVEL, and not a row, which is
/// why a mark's row cannot reach it.
fn open(command: &str, _: Option<&str>, _: &mut dyn Doorway) -> Box<dyn crate::region::Region> {
    let view = if command == "network-tor-open" {
        View::fresh_on_the_tor_sheet()
    } else {
        View::fresh()
    };
    Box::new(NetworkRegion {
        view,
        net: Snapshot::default(),
        tunnels: ronoctua_tunnel::Scene::default(),
        hand: None,
        join: None,
        opened: false,
    })
}

/// The network view, as a region of the footer.
pub struct NetworkRegion {
    view: View,
    /// The last reading of the daemon — the footer's, and this view's own
    /// after every order.
    net: Snapshot,
    /// What the two tunnels say, as the footer heard it.
    tunnels: ronoctua_tunnel::Scene,
    /// The footer's line to the tunnels' thread.
    hand: Option<ronoctua_tunnel::Hand>,
    /// The network being joined, and a password waiting for its "keep it?"
    /// — see [`Held::join`].
    join: Option<Join>,
    /// Whether the command that opened it has run.
    opened: bool,
}

impl NetworkRegion {
    /// The rows, at the door's width.
    fn rows_at(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<crate::network::Row> {
        with_kept(|k| {
            let seen: &[ronoctua_network::Network] = k.wire.as_ref().map_or(&[], Wire::seen);
            self.view.rows(
                &Facts {
                    net: &self.net,
                    seen,
                    watch: &k.watch,
                    tunnels: &self.tunnels,
                },
                columns,
                texts,
            )
        })
    }

    /// An errand, carried out with what this region and the process hold.
    fn carry(&mut self, errand: Errand, k: &mut Kept, way: &mut dyn Doorway) {
        // WHAT IT STARTS COMES BACK TO WHICHEVER OPENING IS ON SCREEN THEN
        // (`Kept::way_back`): this one may be gone by then.
        let back = k.way_back();
        let wake = way.wake();
        let follow = move || crate::network::scans::follow(wake.clone());
        // A PHOTOGRAPH'S DOOR has no loop for a thread to come back to, and
        // reaches nothing of the machine on its own.
        let photographing = way.photographing();
        let lines = Lines {
            lan: (!photographing).then_some(&back as &dyn Fn() -> Back),
            tunnels: self.hand.as_ref(),
            scans: (!photographing).then_some(&follow as &dyn Fn()),
        };
        let wire = k.wire.get_or_insert_with(Wire::fresh);
        let mut held = Held {
            view: &mut self.view,
            wire,
            net: &mut self.net,
            watch: &mut k.watch,
            tunnels: &self.tunnels,
            join: &mut self.join,
        };
        carry_out(errand, &mut held, &lines, way);
    }

    /// The network being joined, followed on a reading of the daemon
    /// ([`follow_join`]).
    fn follow_join(&mut self, way: &mut dyn Doorway) {
        if self.join.is_none() {
            return;
        }
        with_kept(|k| {
            let wire = k.wire.get_or_insert_with(Wire::fresh);
            let mut held = Held {
                view: &mut self.view,
                wire,
                net: &mut self.net,
                watch: &mut k.watch,
                tunnels: &self.tunnels,
                join: &mut self.join,
            };
            follow_join(&mut held, way);
        });
    }

    /// One key of the view, wherever it came from — a key, a command by
    /// name, a click's `RET`.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        // A PRESS THAT REACHES THE VIEW WHILE THE LINE ASKS came from a click
        // — every key is the line's — and the view lets go of what it asked
        // on any press it hears. The line goes with it, as the window's does:
        // left up, it would take a password for a question nobody is asking
        // any more.
        for question in Question::THE_VIEWS {
            way.let_go_of(question.id);
        }
        with_kept(|k| {
            let wire = k.wire.get_or_insert_with(Wire::fresh);
            let errand = self.view.key(
                press,
                &Facts {
                    net: &self.net,
                    seen: wire.seen(),
                    watch: &k.watch,
                    tunnels: &self.tunnels,
                },
                way.columns(),
                way.texts(),
            );
            self.carry(errand, k, way);
        });
    }

    /// What was typed on the footer's line for the view — a password, a
    /// path, a Tor value — handed to the SAME `View::answered` the window's
    /// line reaches, and its errand carried out here.
    fn told(&mut self, id: &str, text: &str, way: &mut dyn Doorway) {
        with_kept(|k| {
            let wire = k.wire.get_or_insert_with(Wire::fresh);
            let errand = self.view.answered(
                id,
                text,
                &Facts {
                    net: &self.net,
                    seen: wire.seen(),
                    watch: &k.watch,
                    tunnels: &self.tunnels,
                },
                way.columns(),
                way.texts(),
            );
            self.carry(errand, k, way);
        });
        way.follow();
    }

    /// The command that opened the view, with the bus and the scan watcher
    /// behind it — on the row a mark was about, when one was.
    ///
    /// THE FIRST SAMPLE IS TAKEN AT OPENING, not on the first tick: the
    /// clock runs once a second, so without this the first frame drew "no
    /// ports", an empty neighbour list and an empty monitor for a whole
    /// second. AND THE RATES START HERE TOO: what `/proc` says outlives the
    /// view, so a second opening an hour later would have divided the bytes
    /// since the last sample by an hour and called it a rate. A NEW VIEW ASKS
    /// NOTHING YET: a password that was waiting for the old one's "keep it?"
    /// goes with it.
    fn opened(&mut self, at: Option<&Target>, way: &mut dyn Doorway) {
        self.opened = true;
        if !way.photographing() {
            crate::network::scans::follow(way.wake());
        }
        // THE FILE PICKER'S FOLDER, watched through the region's own wake.
        self.view.watch_with(way.wake());
        with_kept(|k| {
            // WHAT THE VIEW'S THREADS BRING BACK FROM NOW ON wakes this
            // opening, whoever asked it — set BEFORE what came back already
            // is taken, so an answer landing between the two is read either
            // way.
            k.listen(Some(way.wake()));
            k.take_what_was_heard();
            let wire = k.wire.get_or_insert_with(Wire::fresh);
            if let Some(target) = at {
                wire.reread();
                self.net = wire.look();
                let mut view = View::fresh_at(
                    &Facts {
                        net: &self.net,
                        seen: wire.seen(),
                        watch: &k.watch,
                        tunnels: &self.tunnels,
                    },
                    target,
                    way.columns(),
                    way.texts(),
                );
                view.watch_with(way.wake());
                self.view = view;
            }
            wire.reread();
            self.net = wire.look();
            // A HOTSPOT ALREADY UP shows its password from the first frame,
            // whichever door started it.
            self.view
                .show_sharing(crate::link_words::sharing::serving(&self.net));
            k.watch.restart();
            k.watch.sample();
        });
        self.join = None;
        way.follow();
    }

    /// The rows settle on what is there now: an arrival does not move the
    /// page (`settle_region`), it only keeps the cursor on a row that exists.
    fn settle(&mut self, way: &mut dyn Doorway) {
        let rows = self.rows_at(way.columns(), way.texts());
        self.view.settle(&rows);
    }
}

impl crate::region::Region for NetworkRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE VIEW'S ROWS, as `fields` — two columns of its own. The question
    /// "keep it?" is among them, at the top, drawn by the view for both
    /// doors — and a click on its answers is the first-selects, second-acts
    /// of every row.
    fn rows(&self, seen: &Seen<'_>) -> Vec<crate::region::Row> {
        self.rows_at(seen.columns, seen.texts)
            .into_iter()
            .map(|r| crate::region::Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    /// The radio and the network, whose tables are bare letters: without
    /// the line there is nothing on screen that says `s` scans.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    /// THE QUESTION "KEEP IT?" IS ONE OF THE VIEW'S OWN ROWS, so the cursor
    /// is already the line on screen.
    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE VIEW'S KEYS come out of the SAME table the manual reads —
    /// `network::OF_THE_VIEW` — read by the key's WHOLE notation, through
    /// the one reader the window calls too (`network_words::press_of_key`).
    /// A key outside the table does nothing and does NOT close the view:
    /// closing on a wrong key would lose the place in the list. THE FILE
    /// PICKER, while it is up over the view, takes what it reads — every
    /// letter, and its own keys.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let press = if self.view.picker_takes(key) {
            Press::Pick(key)
        } else {
            match crate::network_words::press_of_key(key) {
                Some(press) => press,
                None => return Taken::No,
            }
        };
        // THE REGION FOLLOWS THE CURSOR: on this machine the view is some
        // eighty rows, and holding `<down>` past the eleventh drew the same
        // screen for ever. AND THE SURFACE FIRST — the ten sections are not
        // the same length (`Doorway::follow`).
        self.press(press, way);
        way.follow();
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let first = !self.opened;
        match (id, argument) {
            // THE COMMAND THAT OPENED IT, the first time; a second open is
            // refused, as every door that asks whether the region is free
            // refuses with it held — here by the view itself, which keeps its
            // place rather than building one that forgot it.
            ("network-open" | "network-tor-open", _) | ("network-import", Some(_)) if first => {
                let at = match (id, argument) {
                    ("network-open", Some(on)) => opened_on(on),
                    _ => None,
                };
                self.opened(at.as_ref(), way);
                // A PATH HANDED IN FROM OUTSIDE opens the view to show it.
                if let ("network-import", Some(path)) = (id, argument) {
                    self.told(id, path, way);
                }
            }
            ("network-open" | "network-tor-open", _) => {
                tracing::info!(
                    view = id,
                    held = ID,
                    "refused: another view is open in the footer"
                );
                let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
                way.say(said, Echo::Normal);
                way.repaint();
            }
            ("network-window", _) => way.open_window(vec!["network".to_owned()]),
            // THE TYPED ANSWERS, by name with an argument — the line's answer
            // in either door, or `LEADER :` — into `View::answered`. With no
            // argument nothing was typed, and nothing changes; but `RET` on
            // the focused network (`network-connect`) and the file's question
            // (`network-import`) are presses of the view.
            (answer, Some(text)) if ANSWERS.contains(&answer) => self.told(answer, text, way),
            ("network-connect", None) => {
                self.press(Press::Enter, way);
                way.follow();
            }
            ("network-import", None) => {
                self.press(Press::Import, way);
                way.follow();
            }
            (answer, None) if ANSWERS.contains(&answer) => {}
            (other, _) => match press_of_command(other) {
                Some(press) => {
                    self.press(press, way);
                    way.follow();
                }
                None => return Taken::No,
            },
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click on
    /// the row the cursor is already on is that row's `RET`.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.rows_at(way.columns(), way.texts());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
            way.follow();
        } else {
            way.repaint();
        }
    }

    /// A CLICK ON A LOCATION'S TRASH GLYPH FORGETS IT — the mouse half of
    /// `DEL`/`f`. A registered location draws a trash glyph as its value
    /// (`View::rows`); the glyph is a field at its own cell, so a click from
    /// that cell rightward is the glyph, and anywhere left of it is the name,
    /// which raises like any list row. Everything that is not that glyph falls
    /// through to the plain row click.
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let rows = self.rows_at(way.columns(), way.texts());
        let on_forget = rows.get(row).is_some_and(|r| {
            matches!(r.target, Target::VpnRaise { .. })
                && r.fields
                    .iter()
                    .find(|f| f.text.starts_with(ronoctua_tokens::icon::TRASH))
                    .zip(cell)
                    .is_some_and(|(glyph, c)| c >= glyph.at)
        });
        if on_forget {
            // Put the cursor on the row, then forget it — the same errand `DEL`
            // raises on a location (`View::key`'s `Press::Forget`).
            self.view.clicked(row, &rows);
            self.press(Press::Forget, way);
            return;
        }
        self.clicked(row, way);
    }

    /// What was typed on the footer's line for the view's question.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.told(id, text, way);
    }

    /// The clipboard's text, for the file picker up over the view.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        self.view.picker_pasted(text);
        way.repaint();
    }

    /// The region's wake rang: the file picker's folder moved or its devices
    /// were read; a sweep, the names or a measurement came back; or the radio
    /// finished a scan — re-read then, with the list on screen.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let mut changed = self.view.look();
        changed |= with_kept(|k| {
            let mut changed = k.take_what_was_heard();
            if crate::network::scans::ended()
                && let Some(w) = k.wire.as_mut()
            {
                w.reread();
                changed = true;
            }
            changed
        });
        if changed {
            self.settle(way);
            way.repaint();
        }
    }

    /// What the footer heard: the daemon's reading (and the hotspot it
    /// shows), the tunnels' reading and the line to their thread. AND THE
    /// CURSOR FOLLOWS THE ROWS THAT ARE LEFT — a cable unplugged takes six
    /// facts away under the cursor's feet; a tailnet going down takes its
    /// whole section.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(net) = news.downcast_ref::<Snapshot>() {
            self.net = net.clone();
            self.view
                .show_sharing(crate::link_words::sharing::serving(&self.net));
            // A READING IS WHEN A JOIN IS SETTLED: the device's state moved.
            self.follow_join(way);
        } else if let Some(tunnels) = news.downcast_ref::<ronoctua_tunnel::Scene>() {
            self.tunnels = tunnels.clone();
        } else if let Some(ronoctua_tunnel::Event::Scene(tunnels)) =
            news.downcast_ref::<ronoctua_tunnel::Event>()
        {
            self.tunnels = (**tunnels).clone();
        } else if let Some(hand) = news.downcast_ref::<ronoctua_tunnel::Hand>() {
            self.hand = Some(hand.clone());
            return;
        } else {
            return;
        }
        self.settle(way);
        way.repaint();
    }

    /// THE SOCKETS AND COUNTERS RIDE THE FOOTER'S SECOND while the view is
    /// open: a rate is a difference over a time, so these numbers cannot come
    /// from the bus's signals. AND THE NAMES OF WHAT IT FOUND, asked of
    /// resolved on a thread — once per address, never on the loop.
    fn tick(&mut self, _: std::time::Instant, way: &mut dyn Doorway) -> bool {
        with_kept(|k| {
            k.watch.sample();
            // AND THE ANSWER COMES BACK TO THE OPENING THERE IS THEN: resolved
            // can take seconds, and an `Esc` and a second opening in between
            // would leave the batch out for ever (`Kept::way_back`).
            if !way.photographing()
                && let Some(list) = k.watch.names_wanted()
            {
                crate::network::spawn(Lan::Name(list), k.back());
            }
        });
        true
    }

    /// The lists inside the list: Wi-Fi, the remembered, the ports, the
    /// neighbours and the peers, each a band with a bar of its own.
    fn shelves(&self, seen: &Seen<'_>) -> Vec<crate::region::Shelf> {
        let rows = self.rows_at(seen.columns, seen.texts);
        self.view
            .shelves(&rows)
            .into_iter()
            .filter_map(|b| {
                Some(crate::region::Shelf {
                    id: SHELVES.iter().position(|s| *s == b.shelf)?,
                    first: b.first,
                    lines: b.lines,
                    total: b.total,
                    offset: b.offset,
                })
            })
            .collect()
    }

    fn shelf_scrolled(&mut self, id: usize, offset: usize, way: &mut dyn Doorway) {
        if let Some(shelf) = SHELVES.get(id) {
            self.view.put_shelf(*shelf, offset);
            way.repaint();
        }
    }

    fn page_is(&self, lines: usize) {
        self.view.page_is(lines);
    }

    /// `TAB` lands on a section and has to SHOW it: its title at the top,
    /// once (`View::take_anchor`).
    fn take_anchor(&mut self) -> Option<usize> {
        self.view.take_anchor()
    }

    /// The view goes, and so does its following of the scans and of its
    /// threads' answers; what the process keeps stays for the next — an
    /// answer that comes now waits in the channel for it.
    fn closing(&mut self, _: &mut dyn Doorway) {
        crate::network::scans::let_go();
        with_kept(|k| k.listen(None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder, Region};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    /// A machine with a radio that is on and the daemon answering.
    fn machine() -> Snapshot {
        use ronoctua_network::{Band, Link, Medium, State, Way};
        Snapshot {
            daemon: true,
            networking: true,
            radio: true,
            way: Way::Full,
            links: vec![Link {
                medium: Medium::Radio,
                interface: "wlan0".into(),
                network: Some("noctua".into()),
                state: State::Connected,
                rate_kbps: 400_000,
                signal: 70,
                band: Band::Other,
                address: Some("10.0.0.2/24".to_owned()),
                gateway: Some("10.0.0.1".to_owned()),
                dns: Vec::new(),
                serving: false,
            }],
            active: Vec::new(),
            saved: Vec::new(),
        }
    }

    /// FAILS IF: a mark's row and the argument it opens the view with stop
    /// agreeing — the tunnel's mark would open the list with the row
    /// somewhere nobody is looking.
    #[test]
    fn a_marks_row_survives_the_command() {
        for target in [
            Target::Tunnel("/org/freedesktop/NetworkManager/ActiveConnection/7".to_owned()),
            Target::Tunnels(Switch::Tailnet, false),
            Target::Tunnels(Switch::Tor, false),
        ] {
            let said = opening_on(&target).expect("a mark's row");
            assert_eq!(opened_on(&said), Some(target));
        }
        assert_eq!(opening_on(&Target::Nothing), None);
        assert_eq!(opened_on("anything else"), None);
    }

    /// FAILS IF: the facts of the link are said in one door and not the
    /// other, or an order re-reads nothing: one handler for both, over the
    /// door's doorway, saying what happened.
    #[test]
    fn an_errand_is_said_through_the_door_and_re_reads() {
        let mut w = way();
        let mut view = View::fresh();
        // THE DAEMON SAYS SOMETHING NEW after the order: the link went.
        let after = Snapshot {
            daemon: true,
            ..Snapshot::default()
        };
        let mut wire = Wire::planted(after.clone(), Vec::new());
        let mut net = machine();
        let mut watch = Watch::default();
        let tunnels = ronoctua_tunnel::Scene::default();
        let mut join = Some(a_join("a café", Some("s3nh4"), true));
        let lines = Lines {
            lan: None,
            tunnels: None,
            scans: None,
        };
        let mut held = Held {
            view: &mut view,
            wire: &mut wire,
            net: &mut net,
            watch: &mut watch,
            tunnels: &tunnels,
            join: &mut join,
        };
        carry_out(Errand::Facts, &mut held, &lines, &mut w);
        assert_eq!(*held.net, after, "the order did not re-read");
        assert!(
            matches!(w.owed.first(), Some(Effect::Say(said, Echo::Normal)) if said.contains("noctua")),
            "{:?}",
            w.owed
        );
        // NO THREAD FOR THE WORK, no tunnels' thread: each says so.
        let mut w = way();
        carry_out(
            Errand::Measure(std::net::Ipv4Addr::new(10, 0, 0, 1)),
            &mut held,
            &lines,
            &mut w,
        );
        carry_out(Errand::TorAutostart(true), &mut held, &lines, &mut w);
        let unavailable = w.texts().t(crate::key::ECHO_NETWORK_UNAVAILABLE);
        let says: Vec<&Effect> = w
            .owed
            .iter()
            .filter(|e| matches!(e, Effect::Say(..)))
            .collect();
        assert_eq!(
            says,
            vec![
                &Effect::Say(unavailable.clone(), Echo::Normal),
                &Effect::Say(unavailable, Echo::Normal)
            ]
        );
        // A CLOSE LETS GO OF THE PASSWORD that waited for "keep it?".
        let mut w = way();
        carry_out(Errand::Close, &mut held, &lines, &mut w);
        assert!(held.join.is_none(), "a password outlived its view");
        assert_eq!(w.owed, vec![Effect::Close]);
    }

    /// A join a door holds, with no daemon behind it.
    fn a_join(ssid: &str, password: Option<&str>, up: bool) -> Join {
        Join {
            ssid: ssid.to_owned(),
            password: password.map(Password::typed),
            joining: ronoctua_network::Joining::at(
                "/org/freedesktop/NetworkManager/Settings/9",
                "/org/freedesktop/NetworkManager/ActiveConnection/9",
                true,
            )
            .unwrap(),
            up,
        }
    }

    /// FAILS IF: a door says "connected", or asks "keep it?", before
    /// NetworkManager says the network came up — the question would seal a
    /// password the network may refuse — or a refusal keeps the password, or
    /// is not said; or a saved network brought up asks to keep a password it
    /// never had typed; or "yes" before the network is up seals anything.
    #[test]
    fn a_join_is_said_and_kept_only_once_it_is_up() {
        use ronoctua_network::Joined;
        let lines = Lines {
            lan: None,
            tunnels: None,
            scans: None,
        };
        // What one reading does to a join, or what one errand does to it.
        let run = |verdict: Joined, password: Option<&str>, errand: Option<Errand>| {
            let mut w = way();
            let mut view = View::fresh();
            let mut wire = Wire::offline(Vec::new()).joining_as(verdict);
            let mut net = machine();
            let mut watch = Watch::default();
            let tunnels = ronoctua_tunnel::Scene::default();
            let mut join = Some(a_join("a café", password, false));
            let mut held = Held {
                view: &mut view,
                wire: &mut wire,
                net: &mut net,
                watch: &mut watch,
                tunnels: &tunnels,
                join: &mut join,
            };
            match errand {
                Some(e) => carry_out(e, &mut held, &lines, &mut w),
                None => follow_join(&mut held, &mut w),
            }
            let said: Vec<String> = w
                .owed
                .iter()
                .filter_map(|e| match e {
                    Effect::Say(s, _) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            (said, view.remembering(), join)
        };
        // ON ITS WAY: nothing said, nothing asked, the password still held.
        let (said, asked, join) = run(Joined::Coming, Some("s3nh4"), None);
        assert!(said.is_empty(), "{said:?}");
        assert!(!asked);
        assert!(join.is_some_and(|j| !j.up && j.password.is_some()));
        // UP: "connected", then "keep it?", the password held for it.
        let (said, asked, join) = run(Joined::Up, Some("s3nh4"), None);
        assert!(
            said.len() == 1 && said[0].contains("a café") && !said[0].contains("s3nh4"),
            "{said:?}"
        );
        assert!(asked, "the question did not come once the network was up");
        assert!(join.is_some_and(|j| j.up));
        // REFUSED: said, as the key's, and the password let go.
        let wrong_key = {
            let why = anyhow::Error::from(ronoctua_network::refusal::Named(
                ronoctua_network::refusal::Refusal::NoSecrets,
            ));
            crate::network_refusal::reason(&why, way().texts())
        };
        let (said, asked, join) = run(Joined::Refused, Some("s3nh4"), None);
        assert!(said.len() == 1 && said[0].contains("a café"), "{said:?}");
        assert!(
            said[0].contains(&wrong_key),
            "a refused key not said as one: {said:?}"
        );
        assert!(!asked);
        assert!(join.is_none(), "a refused password was kept");
        // FAILED for another reason: said, never as a wrong password.
        let (said, asked, join) = run(Joined::Failed, Some("s3nh4"), None);
        assert!(said.len() == 1 && said[0].contains("a café"), "{said:?}");
        assert!(
            !said[0].contains(&wrong_key),
            "a network that never came up said as a wrong key: {said:?}"
        );
        assert!(!asked);
        assert!(join.is_none(), "a failed join's password was kept");
        // A SAVED NETWORK brought up has nothing to keep.
        let (said, asked, join) = run(Joined::Up, None, None);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(!asked, "a saved network asked to keep a password");
        assert!(join.is_none());
        // AND "YES" BEFORE IT IS UP seals nothing: there is nothing to answer.
        let yes = Errand::Remember("a café".into(), true);
        let (said, _, join) = run(Joined::Coming, Some("s3nh4"), Some(yes));
        let unavailable = way().texts().t(crate::key::ECHO_NETWORK_UNAVAILABLE);
        assert_eq!(said, vec![unavailable]);
        assert!(join.is_none(), "the password stayed after the answer");
    }

    /// FAILS IF: a press that reaches the view while the line asks one of its
    /// questions leaves the line up — a password typed next would be for a
    /// question nobody is asking.
    #[test]
    fn a_press_takes_the_views_question_down() {
        let mut w = way();
        let mut r = NetworkRegion {
            view: View::fresh(),
            net: machine(),
            tunnels: ronoctua_tunnel::Scene::default(),
            hand: None,
            join: None,
            opened: true,
        };
        with_kept(|k| k.wire = Some(Wire::planted(machine(), Vec::new())));
        r.press(Press::Down, &mut w);
        for q in Question::THE_VIEWS {
            assert!(
                w.owed.contains(&Effect::LetGoOf(q.id)),
                "the line asking {} stays",
                q.id
            );
        }
    }

    /// FAILS IF: the command that opened the view is refused as a second
    /// open, or does not settle the view on a fresh reading, or a second open
    /// is not refused.
    #[test]
    fn the_first_open_is_the_opening_and_a_second_is_refused() {
        with_kept(|k| k.wire = Some(Wire::planted(machine(), Vec::new())));
        let mut w = way();
        let mut r = NetworkRegion {
            view: View::fresh(),
            net: Snapshot::default(),
            tunnels: ronoctua_tunnel::Scene::default(),
            hand: None,
            join: Some(a_join("a café", Some("s3nh4"), true)),
            opened: false,
        };
        assert_eq!(r.command("network-open", None, &mut w), Taken::Yes);
        assert_eq!(r.net, machine(), "the opening read nothing");
        assert!(
            r.join.is_none(),
            "a password outlived the view it was asked in"
        );
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "the opening said something: {:?}",
            w.owed
        );
        let mut w = way();
        assert_eq!(r.command("network-open", None, &mut w), Taken::Yes);
        let busy = w.texts().t(crate::key::ECHO_VIEW_BUSY);
        assert_eq!(w.owed.first(), Some(&Effect::Say(busy, Echo::Normal)));
    }

    /// FAILS IF: the shelves the footer draws a bar beside do not come back
    /// as the view's own when one is dragged.
    #[test]
    fn a_shelf_is_known_both_ways() {
        for (n, shelf) in SHELVES.iter().enumerate() {
            assert_eq!(SHELVES.iter().position(|s| s == shelf), Some(n));
        }
        let mut w = way();
        let mut r = NetworkRegion {
            view: View::fresh(),
            net: machine(),
            tunnels: ronoctua_tunnel::Scene::default(),
            hand: None,
            join: None,
            opened: true,
        };
        r.shelf_scrolled(SHELVES.len(), 3, &mut w);
        assert!(w.owed.is_empty(), "a shelf nobody has was scrolled");
        r.shelf_scrolled(0, 3, &mut w);
        assert_eq!(w.owed, vec![Effect::Repaint]);
    }

    /// FAILS IF: a gesture of the view by name is not taken with the view
    /// closed (`LEADER s n s` from anywhere), a typed answer for no view is
    /// taken by anyone else, or what opens the view stops opening it.
    #[test]
    fn the_closed_view_answers_by_name() {
        with_kept(|k| k.wire = Some(Wire::planted(machine(), Vec::new())));
        let mut w = way();
        assert_eq!(closed("network-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("network-tor-open", None, &mut w), Closed::Opens);
        assert_eq!(
            closed("network-import", Some("/x.conf"), &mut w),
            Closed::Opens
        );
        assert_eq!(
            closed("network-vpn-user", Some("ana"), &mut w),
            Closed::Taken
        );
        assert!(w.owed.is_empty(), "an answer for no view did something");
        assert_eq!(closed("network-link", None, &mut w), Closed::Taken);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(said, _) if said.contains("noctua"))),
            "the link's facts were not said with the view closed: {:?}",
            w.owed
        );
        assert_eq!(closed("bluetooth-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: what a thread of the view brings back after the door that
    /// asked it is gone — a sweep asked with the view closed, a name asked by
    /// an opening before `Esc` and a second opening — wakes nobody, or wakes
    /// that door instead of the opening on screen now: it would sit unread
    /// for the whole opening, the shelf saying a sweep runs and the address
    /// saying "asking", and the names that batch held up never asked.
    #[test]
    fn an_answer_wakes_the_opening_on_screen_when_it_comes() {
        use crate::network::Named;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A doorway whose wake counts how often it rang.
        fn counted() -> (Arc<AtomicUsize>, Recorder) {
            let rang = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&rang);
            let texts =
                ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
            let way = Recorder::new(
                std::rc::Rc::new(texts),
                80,
                true,
                ronoctua_shell_door::sudo::Gate::window(),
                ronoctua_shell_kit::wake::Wake::new(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                }),
            );
            (rang, way)
        }
        fn named(last: u8, name: &str) -> (std::net::IpAddr, Heard) {
            let at = std::net::IpAddr::from([10, 0, 0, last]);
            (at, Heard::Named(vec![(at, Ok(Some(name.to_owned())))]))
        }
        fn known(at: std::net::IpAddr) -> Option<Named> {
            with_kept(|k| k.watch.name_of(at).cloned())
        }
        let rang = |r: &Arc<AtomicUsize>| r.load(Ordering::SeqCst);

        with_kept(|k| k.wire = Some(Wire::planted(machine(), Vec::new())));
        // ASKED WITH THE VIEW CLOSED — two of them; one comes back while it
        // still is, with nobody to wake.
        let early = with_kept(|k| k.back());
        let late = with_kept(|k| k.back());
        let (printer, said) = named(7, "printer");
        early(said);

        // THE FIRST OPENING takes what waited, as it opens.
        let (first_rang, mut first_way) = counted();
        let mut first = open("network-open", None, &mut first_way);
        assert_eq!(
            first.command("network-open", None, &mut first_way),
            Taken::Yes
        );
        assert_eq!(known(printer), Some(Named::Known("printer".to_owned())));

        // ASKED BEFORE IT OPENED, back after: this opening is woken, and takes it.
        let (phone, said) = named(8, "phone");
        late(said);
        assert_eq!(rang(&first_rang), 1, "the opening on screen was not woken");
        first.woken(&mut first_way);
        assert_eq!(known(phone), Some(Named::Known("phone".to_owned())));

        // ASKED BY THIS OPENING, back after `Esc` and a second opening: the
        // second is woken, and the first — gone — is not.
        let asked_by_the_first = with_kept(|k| k.back());
        first.closing(&mut first_way);
        drop(first);
        let (second_rang, mut second_way) = counted();
        let mut second = open("network-open", None, &mut second_way);
        assert_eq!(
            second.command("network-open", None, &mut second_way),
            Taken::Yes
        );
        let (tv, said) = named(9, "tv");
        asked_by_the_first(said);
        assert_eq!(rang(&first_rang), 1, "the opening that is gone was woken");
        assert_eq!(rang(&second_rang), 1, "the opening on screen was not woken");
        second.woken(&mut second_way);
        assert_eq!(known(tv), Some(Named::Known("tv".to_owned())));

        // AND WITH THE VIEW CLOSED AGAIN nothing is woken: the answer waits.
        second.closing(&mut second_way);
        let (_, said) = named(10, "kettle");
        with_kept(|k| k.back())(said);
        assert_eq!(rang(&second_rang), 1, "a closed view was woken");
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, a sentence, a shelf's blank — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a view of its own, and lit exactly where something
    /// happened. Safe to click through: the process keeps a wire with no
    /// daemon (every order is refused), the region has no tunnel thread, and
    /// a photograph's doorway starts no sweep, scan or measurement — so the
    /// second click's `RET` reaches nothing of the machine.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        // THE ZONE BY ITS DEFAULT: the view writes no date, and this crate
        // names no `ronoctua-files` to ask for UTC by name.
        let zone = Default::default();
        let seen = Seen {
            texts: &t,
            // `way()`'s width: a click builds the rows at the doorway's.
            columns: 80,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let fresh = || {
            with_kept(|k| k.wire = Some(Wire::planted(machine(), Vec::new())));
            let r = NetworkRegion {
                view: View::fresh(),
                net: machine(),
                tunnels: ronoctua_tunnel::Scene::default(),
                hand: None,
                join: None,
                opened: true,
            };
            (r, way())
        };
        let held = crate::region::held_to_the_click(fresh, &seen, |r, w: &Recorder| {
            crate::region::seen(r, &w.owed, &seen)
        });
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
