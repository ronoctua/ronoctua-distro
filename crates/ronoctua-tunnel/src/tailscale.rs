//! Tailscale, through the local API `tailscaled` already exposes.
//!
//! # Where the socket is, and who may open it
//!
//! `/run/tailscale/tailscaled.sock`, owned by root. An ordinary user reaches
//! it because the service is started with `--operator=<user>`, which is what
//! `nix/modules/tailscale.nix` writes — there is no polkit action here to
//! widen, and no group to be in.
//!
//! # Why every field is optional
//!
//! The local API is not a stable interface: it is what the `tailscale` command
//! itself talks to, and it changes with the daemon. Every field is read with
//! `#[serde(default)]`, so a release that renames one takes a fact off the
//! screen instead of emptying the whole section — and the test below is a
//! fixture of the SHAPE, with no real machine, tailnet or key in it
//! (invariant 13).

use std::path::Path;

use serde::Deserialize;

/// The `Host` header the daemon insists on. A request with any other one
/// comes back 403, which reads exactly like a permission problem.
const HOST: &str = "local-tailscaled.sock";

/// Where the daemon listens, unless the environment says otherwise.
pub const SOCKET: &str = match option_env!("RONOCTUA_TAILSCALE_SOCKET") {
    Some(s) => s,
    None => "/run/tailscale/tailscaled.sock",
};

/// One machine on the tailnet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peer {
    /// The node's stable id — what an exit node is chosen by.
    pub id: String,
    /// What to call it on screen.
    pub name: String,
    /// `linux`, `macOS`, `iOS`.
    pub os: String,
    /// Is it reachable now?
    pub online: bool,
    /// Its addresses on the tailnet.
    pub addresses: Vec<String>,
    /// Does it offer to be an exit node?
    pub offers_exit: bool,
    /// Is it the exit node in use?
    pub exit_node: bool,
    /// Bytes in and out since the daemon came up.
    pub received: u64,
    /// Likewise out.
    pub sent: u64,
}

/// The whole of what Tailscale says, as of one reading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scene {
    /// `Running`, `NeedsLogin`, `Stopped`, `NoState`.
    pub state: String,
    /// Is the tunnel up AND this machine online on it?
    pub running: bool,
    /// This machine's name on the tailnet.
    pub name: String,
    /// The tailnet's own name.
    pub tailnet: String,
    /// This machine's addresses.
    pub addresses: Vec<String>,
    /// Is MagicDNS on?
    pub magic_dns: bool,
    /// The url to open when the daemon wants a login.
    pub auth_url: Option<String>,
    /// Does the person want it running? It is the switch `up`/`down` writes,
    /// and it is not the same question as `running`: a machine that wants to
    /// run and cannot is exactly the case worth drawing.
    pub wanted: bool,
    /// Are the tailnet's routes accepted?
    pub accept_routes: bool,
    /// Is this machine refusing incoming connections?
    pub shields_up: bool,
    /// The exit node in use, by id.
    pub exit_node: Option<String>,
    /// The other machines.
    pub peers: Vec<Peer>,
}

impl Scene {
    /// Is there anything worth a mark on the modeline?
    ///
    /// RUNNING AND ONLINE, and nothing else: a daemon installed and never
    /// logged into draws nothing, which is the owner's rule for the whole
    /// modeline — nothing at rest.
    #[must_use]
    pub fn marked(&self) -> bool {
        self.running
    }
}

/// `null` READS AS ABSENT, for every field.
///
/// THE DEFECT THIS EXISTS FOR, found by `tests/network-vm.nix`: a daemon that
/// has never joined a tailnet answers `"Peer": null` and `"Self": null`, and
/// serde refuses a `null` where a map or a string is declared — `#[serde(
/// default)]` covers a field that is MISSING, not one that is there and
/// empty. The whole reading then failed, and a daemon that was plainly
/// running was reported as not being there at all.
fn or_default<'de, D, T>(reader: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Ok(Option::deserialize(reader)?.unwrap_or_default())
}

/// What the local API answers to `/status`, as much of it as is read.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawStatus {
    #[serde(rename = "BackendState", deserialize_with = "or_default")]
    backend_state: String,
    #[serde(rename = "AuthURL", deserialize_with = "or_default")]
    auth_url: String,
    #[serde(rename = "MagicDNSSuffix", deserialize_with = "or_default")]
    magic_dns_suffix: String,
    #[serde(rename = "CurrentTailnet", deserialize_with = "or_default")]
    tailnet: Option<RawTailnet>,
    #[serde(rename = "Self", deserialize_with = "or_default")]
    this: Option<RawNode>,
    #[serde(rename = "Peer", deserialize_with = "or_default")]
    peers: std::collections::BTreeMap<String, RawNode>,
    #[serde(rename = "ExitNodeStatus", deserialize_with = "or_default")]
    exit_node: Option<RawExit>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawTailnet {
    #[serde(rename = "Name", deserialize_with = "or_default")]
    name: String,
    #[serde(rename = "MagicDNSEnabled", deserialize_with = "or_default")]
    magic_dns: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawExit {
    #[serde(rename = "ID", deserialize_with = "or_default")]
    id: String,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
struct RawNode {
    #[serde(rename = "ID", deserialize_with = "or_default")]
    id: String,
    #[serde(rename = "HostName", deserialize_with = "or_default")]
    host_name: String,
    #[serde(rename = "DNSName", deserialize_with = "or_default")]
    dns_name: String,
    #[serde(rename = "OS", deserialize_with = "or_default")]
    os: String,
    #[serde(rename = "Online", deserialize_with = "or_default")]
    online: bool,
    #[serde(rename = "TailscaleIPs", deserialize_with = "or_default")]
    addresses: Vec<String>,
    #[serde(rename = "ExitNodeOption", deserialize_with = "or_default")]
    offers_exit: bool,
    #[serde(rename = "ExitNode", deserialize_with = "or_default")]
    exit_node: bool,
    #[serde(rename = "RxBytes", deserialize_with = "or_default")]
    received: u64,
    #[serde(rename = "TxBytes", deserialize_with = "or_default")]
    sent: u64,
}

/// What the local API answers to `/prefs`, as much of it as is read.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawPrefs {
    #[serde(rename = "WantRunning", deserialize_with = "or_default")]
    want_running: bool,
    #[serde(rename = "RouteAll", deserialize_with = "or_default")]
    route_all: bool,
    #[serde(rename = "ShieldsUp", deserialize_with = "or_default")]
    shields_up: bool,
    #[serde(rename = "ExitNodeID", deserialize_with = "or_default")]
    exit_node: String,
}

/// A name for a node: its DNS name without the tailnet's suffix, or the host
/// name when there is none.
#[must_use]
fn named(node: &RawNode, suffix: &str) -> String {
    let dns = node.dns_name.trim_end_matches('.');
    let short = if suffix.is_empty() {
        dns
    } else {
        dns.trim_end_matches(suffix).trim_end_matches('.')
    };
    if short.is_empty() {
        node.host_name.clone()
    } else {
        short.to_owned()
    }
}

/// The whole scene, from two calls.
///
/// PURE ONCE THE TEXT IS IN: [`scene_of`] is what the test exercises, and it
/// is where every rule lives.
#[must_use]
pub fn scene_of(status: &str, prefs: &str) -> Option<Scene> {
    let status: RawStatus = serde_json::from_str(status).ok()?;
    // PREFS ARE OPTIONAL: a daemon that answers the status and refuses the
    // preferences is still worth drawing, and the switches simply read false.
    let prefs: RawPrefs = serde_json::from_str(prefs).unwrap_or_default();
    let suffix = status.magic_dns_suffix.clone();
    let this = status.this.clone().unwrap_or_default();
    let mut peers: Vec<Peer> = status
        .peers
        .values()
        .map(|p| Peer {
            id: p.id.clone(),
            name: named(p, &suffix),
            os: p.os.clone(),
            online: p.online,
            addresses: p.addresses.clone(),
            offers_exit: p.offers_exit,
            exit_node: p.exit_node,
            received: p.received,
            sent: p.sent,
        })
        .collect();
    // ONLINE FIRST, then by name: the ones that can be reached are the ones
    // worth reading, and the daemon's own order is a map's.
    peers.sort_by(|a, b| b.online.cmp(&a.online).then(a.name.cmp(&b.name)));
    Some(Scene {
        running: status.backend_state == "Running" && this.online,
        state: status.backend_state,
        name: named(&this, &suffix),
        tailnet: status
            .tailnet
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default(),
        magic_dns: status.tailnet.as_ref().is_some_and(|t| t.magic_dns),
        addresses: this.addresses.clone(),
        auth_url: Some(status.auth_url).filter(|u| !u.is_empty()),
        wanted: prefs.want_running,
        accept_routes: prefs.route_all,
        shields_up: prefs.shields_up,
        exit_node: status
            .exit_node
            .map(|e| e.id)
            .filter(|id| !id.is_empty())
            .or_else(|| Some(prefs.exit_node).filter(|id| !id.is_empty())),
        peers,
    })
}

/// Reads the daemon. `None` when there is none — which is the ordinary case
/// on a machine that never logged in, and is drawn as nothing at all.
#[must_use]
pub fn look(socket: &Path) -> Option<Scene> {
    let status = ask(socket, "GET", "/localapi/v0/status", None).ok()?;
    let prefs = ask(socket, "GET", "/localapi/v0/prefs", None).unwrap_or_default();
    scene_of(&status, &prefs)
}

/// One call to the local API.
///
/// The framing is [`ronoctua_http`]'s; what belongs to Tailscale and stays
/// here is the `Host` it insists on and the rule that only a 2xx is an answer.
fn ask(socket: &Path, method: &str, path: &str, body: Option<&str>) -> anyhow::Result<String> {
    let mut call = ronoctua_http::Call::new(method, path, HOST);
    if let Some(body) = body {
        call = call.with_body(body);
    }
    ronoctua_http::over_unix(socket, &call)?.ok()
}

/// One masked preference, which is how the local API takes a change: the
/// field, and a `<field>Set` beside it saying that this one is meant.
fn masked(field: &str, value: &str) -> String {
    format!("{{\"{field}\":{value},\"{field}Set\":true}}")
}

/// Writes one preference.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn set(socket: &Path, field: &str, value: &str) -> anyhow::Result<()> {
    ask(
        socket,
        "PATCH",
        "/localapi/v0/prefs",
        Some(&masked(field, value)),
    )
    .map(|_| ())
}

/// Brings the tunnel up or down.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn set_running(socket: &Path, on: bool) -> anyhow::Result<()> {
    set(socket, "WantRunning", if on { "true" } else { "false" })
}

/// Accepts the tailnet's routes, or stops.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn set_accept_routes(socket: &Path, on: bool) -> anyhow::Result<()> {
    set(socket, "RouteAll", if on { "true" } else { "false" })
}

/// Refuses incoming connections, or stops.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn set_shields_up(socket: &Path, on: bool) -> anyhow::Result<()> {
    set(socket, "ShieldsUp", if on { "true" } else { "false" })
}

/// Sends the traffic through one node, or through none.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn set_exit_node(socket: &Path, id: Option<&str>) -> anyhow::Result<()> {
    // THE ID IS ENCODED AND NOT INTERPOLATED. A node id comes from the
    // daemon, not from us — but writing somebody else's string into JSON with
    // a `format!` is how a quotation mark in it becomes a different request,
    // and this one is the request that decides where all this machine's
    // traffic goes.
    let value = serde_json::to_string(id.unwrap_or(""))?;
    set(socket, "ExitNodeID", &value)
}

/// Asks the daemon to start a login, and gives back the url to open.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn login(socket: &Path) -> anyhow::Result<Option<String>> {
    ask(socket, "POST", "/localapi/v0/login-interactive", None)?;
    // THE URL ARRIVES IN THE STATUS, not in the answer to this call: the
    // daemon starts the login and publishes the address when it has one —
    // which takes a moment, because it has to reach the coordination server
    // first. Asked once and immediately, this raced and came back with
    // nothing, and the door then said the login had started with no address
    // to open. Asked a few times over a couple of seconds, it either has one
    // or the row keeps saying so.
    for _ in 0..10 {
        if let Some(url) = look(socket).and_then(|s| s.auth_url) {
            return Ok(Some(url));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(None)
}

/// Logs the machine out of the tailnet.
///
/// # Errors
///
/// The socket is not there, or the daemon refused.
pub fn logout(socket: &Path) -> anyhow::Result<()> {
    ask(socket, "POST", "/localapi/v0/logout", None).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE SHAPE, and nothing real in it (invariant 13): the names, the
    /// addresses and the keys are invented, and what is being proved is the
    /// grammar the daemon answers in.
    const STATUS: &str = r#"{
      "BackendState": "Running",
      "AuthURL": "",
      "MagicDNSSuffix": "a-tailnet.ts.net",
      "CurrentTailnet": { "Name": "a tailnet", "MagicDNSEnabled": true },
      "Self": {
        "ID": "n1", "HostName": "this-one", "DNSName": "this-one.a-tailnet.ts.net.",
        "OS": "linux", "Online": true, "TailscaleIPs": ["100.64.0.1"],
        "RxBytes": 1024, "TxBytes": 2048
      },
      "Peer": {
        "key-b": {
          "ID": "n2", "HostName": "the-other", "DNSName": "the-other.a-tailnet.ts.net.",
          "OS": "iOS", "Online": false, "TailscaleIPs": ["100.64.0.2"]
        },
        "key-c": {
          "ID": "n3", "HostName": "a-relay", "DNSName": "a-relay.a-tailnet.ts.net.",
          "OS": "linux", "Online": true, "TailscaleIPs": ["100.64.0.3"],
          "ExitNodeOption": true
        }
      }
    }"#;

    const PREFS: &str =
        r#"{ "WantRunning": true, "RouteAll": true, "ShieldsUp": false, "ExitNodeID": "" }"#;

    #[test]
    fn the_scene_comes_out_of_the_two_answers() {
        let s = scene_of(STATUS, PREFS).unwrap();
        assert!(s.running);
        assert!(s.marked());
        assert_eq!(s.name, "this-one", "the suffix is not part of the name");
        assert_eq!(s.tailnet, "a tailnet");
        assert!(s.magic_dns);
        assert_eq!(s.addresses, ["100.64.0.1"]);
        assert!(s.wanted);
        assert!(s.accept_routes);
        assert!(!s.shields_up);
        assert_eq!(s.exit_node, None);
        assert_eq!(s.auth_url, None, "an empty url is no url at all");
        // ONLINE FIRST, then by name.
        let names: Vec<&str> = s.peers.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["a-relay", "the-other"]);
        assert!(s.peers[0].offers_exit);
        assert!(!s.peers[1].online);
    }

    #[test]
    fn a_daemon_that_is_not_logged_in_draws_no_mark() {
        // NOTHING AT REST is the modeline's rule, and a tailnet nobody has
        // joined is exactly rest.
        let waiting = r#"{ "BackendState": "NeedsLogin", "AuthURL": "https://example.invalid/a" }"#;
        let s = scene_of(waiting, "{}").unwrap();
        assert!(!s.running);
        assert!(!s.marked());
        assert_eq!(s.state, "NeedsLogin");
        assert_eq!(s.auth_url.as_deref(), Some("https://example.invalid/a"));
        assert!(s.peers.is_empty());
    }

    #[test]
    fn a_null_reads_as_absent_and_not_as_a_broken_answer() {
        // THE ANSWER A DAEMON THAT NEVER JOINED A TAILNET GIVES, and the
        // defect this test exists for: `#[serde(default)]` covers a field
        // that is MISSING, not one that is there and `null`. The whole
        // reading failed and a daemon that was plainly running was reported
        // as not being there — found by `tests/network-vm.nix`.
        let waiting = r#"{
          "BackendState": "NeedsLogin",
          "AuthURL": null,
          "MagicDNSSuffix": null,
          "CurrentTailnet": null,
          "Self": null,
          "Peer": null,
          "ExitNodeStatus": null
        }"#;
        let s = scene_of(waiting, "null").expect("a null took the whole reading");
        assert_eq!(s.state, "NeedsLogin");
        assert!(!s.running);
        assert!(s.peers.is_empty());
        assert_eq!(s.auth_url, None);
        assert_eq!(s.tailnet, "");
    }

    #[test]
    fn a_field_that_went_away_takes_one_fact_and_not_the_screen() {
        // THE LOCAL API IS NOT STABLE. A release that renames `RouteAll` must
        // cost this view one switch, not the whole section.
        let s = scene_of(r#"{"BackendState":"Running","Self":{"Online":true}}"#, "{}").unwrap();
        assert!(s.running);
        assert!(!s.accept_routes);
        assert_eq!(s.tailnet, "");
        // AND A BODY THAT IS NOT JSON AT ALL is no scene, rather than a scene
        // full of defaults that would read as "logged out".
        assert!(scene_of("<html>403</html>", "{}").is_none());
    }

    #[test]
    fn an_exit_node_is_encoded_and_not_interpolated() {
        // A quotation mark in an id would have closed the string and left the
        // rest as JSON of its own — on the one request that decides where all
        // this machine's traffic goes.
        let quoted = serde_json::to_string("n\"3").unwrap();
        assert_eq!(quoted, "\"n\\\"3\"");
        assert_eq!(
            masked("ExitNodeID", &quoted),
            "{\"ExitNodeID\":\"n\\\"3\",\"ExitNodeIDSet\":true}"
        );
    }

    #[test]
    fn a_preference_travels_with_its_mask() {
        // WITHOUT THE MASK the daemon takes the whole struct as given, and
        // every field this crate did not send is written as its zero — which
        // logs the machine out while turning one switch.
        assert_eq!(
            masked("WantRunning", "false"),
            "{\"WantRunning\":false,\"WantRunningSet\":true}"
        );
        assert_eq!(
            masked("ExitNodeID", "\"n3\""),
            "{\"ExitNodeID\":\"n3\",\"ExitNodeIDSet\":true}"
        );
    }
}
