//! `ronoctua-shell-network` — the network view (`LEADER s n`).
//!
//! Everything the view is: what it knows ([`network`]), what it says
//! ([`network_words`]), its window ([`network_window`]) and its region of the
//! footer ([`network_region`]) — the two doors, each carrying out the view's
//! errands through the one handler ([`network_region::carry_out`]). A crate of
//! its own so a line of it recompiles it and the binary, and nothing of the
//! footer or of any other view. What the footer says and reads of the way out
//! too — a tunnel's refusal, the hotspot's password file, the login page's
//! opener, the words a mark opens the view on its row with — is the leaf
//! under both (`ronoctua-shell-network-words`).

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, file_picker, photo, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, echo, fields, line, network_refusal, sizes, wake,
};
// THE WAY OUT AS THIS VIEW AND THE FOOTER SAY IT: a leaf under both.
use ronoctua_shell_network_words::link_words;

pub mod key;
pub mod network;
pub mod network_region;
pub mod network_window;
pub mod network_words;

/// THE NETWORK'S DOORS the binary runs by name: the network in a window of
/// its own (`ronoctua-shell network`), and what the view would draw as lines
/// a test can read (`network-facts`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "network",
        run: network_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "network-facts",
        run: facts_door,
    },
];

/// THE NETWORK WINDOW'S CAMERA (`photograph --door network`), over a planted
/// machine.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "network",
    take: network_window::camera,
}];

/// `ronoctua-shell network` — the network in a window of its own. The SAME
/// `network::View` the footer draws from, and it asks what the footer asks —
/// a password masked, a tunnel's path, a Tor value — on its own header's
/// line (`Ui::ask`), where the footer asks on its minibuffer; both answers
/// reach `network::View::answered`.
fn network_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    network_window::host_it().context("host the network view")
}

/// `ronoctua-shell network-facts` — what the view would draw, as lines a test
/// can read.
///
/// IT EXISTS FOR THE VM, and it is honest about that: a virtual machine has
/// no screen worth photographing and no radio at all, so what it can prove
/// about this view is the READINGS — the daemon's answer, the aeroplane,
/// whether the two tunnels are there, how many sockets are listening. Each
/// line is `key=value`, which is what a test asserts on and what a person
/// reads when the screen says something surprising.
#[allow(clippy::unnecessary_wraps)]
fn facts_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
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
    Ok(())
}
