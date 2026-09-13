//! The only module that knows a `/rest/…` path exists.
//!
//! # Where the address and the key come from
//!
//! Out of Syncthing's own `config.xml`, and out of nothing else.
//!
//! This used to be two build-time constants — one for the address, one for the
//! file — and the file's fallback was the literal
//! `/home/noctua/.local/state/syncthing/config.xml`. That broke invariant 16
//! (the owner's name is defined in `nix/modules/base.nix` and nowhere else),
//! and the doc comment above it claimed Nix passed the value through the build
//! when Nix passed nothing at all: the two sides agreed only because two
//! literals happened to match.
//!
//! Reading the file answers all of it. The directory is the XDG state home,
//! which is what `nix/modules/syncthing.nix` computes as well — the same answer
//! from the same rule instead of the same answer written twice. The address is
//! the `<address>` Syncthing itself wrote, so the two cannot drift even if the
//! module's option changes. And nothing anywhere names a user.
//!
//! `RONOCTUA_SYNCTHING_CONFIG` overrides the directory. It is read at RUN time
//! and not baked, because the thing that needs it is a second, throwaway
//! Syncthing on another port — the one every screenshot and every test talks
//! to, so that no measurement of ours ever touches the owner's real folders.
//!
//! # The key never crosses to the screen
//!
//! It is read to sign a request and for nothing else. No caller can ask for it:
//! [`key`] is private, and every public entry point here takes a path and gives
//! back a body.

use std::path::PathBuf;
use std::time::Duration;

/// The environment variable that moves the whole conversation elsewhere.
pub const DIRECTORY_ENV: &str = "RONOCTUA_SYNCTHING_CONFIG";

/// How long the event long poll asks Syncthing to hold the connection.
///
/// A minute. Shorter is a wake-up per period for nothing; longer starts
/// meeting the idle timeouts of things between here and there — and there is
/// nothing between here and there, but a value nobody can defend is a value
/// somebody will change for the wrong reason.
pub const HOLD: Duration = Duration::from_secs(60);

/// How long we wait for the long poll, which must exceed [`HOLD`].
///
/// If this were shorter, the deadline would fire before Syncthing answered and
/// every single poll would read as a service that stopped answering — a view
/// permanently saying the daemon is down while it is up and idle.
pub const HOLD_PATIENCE: Duration = Duration::from_secs(75);

/// Where Syncthing keeps its configuration.
///
/// `$RONOCTUA_SYNCTHING_CONFIG` when it is set; otherwise the XDG state home's
/// `syncthing`, which is the same directory `nix/modules/syncthing.nix` hands
/// the service.
#[must_use]
pub fn home() -> Option<PathBuf> {
    if let Some(set) = std::env::var_os(DIRECTORY_ENV) {
        let set = PathBuf::from(set);
        if !set.as_os_str().is_empty() {
            return Some(set);
        }
    }
    // `state_dir()` is `<state home>/ronoctua`; Syncthing's sibling is
    // `<state home>/syncthing`. Derived from the one place that knows the
    // roots, rather than reading the environment a second time.
    ronoctua_store::state_dir()?
        .parent()
        .map(|state| state.join("syncthing"))
}

/// The `config.xml`, read whole.
fn configuration() -> Option<String> {
    std::fs::read_to_string(home()?.join("config.xml")).ok()
}

/// The text between `<tag>` and `</tag>`, first occurrence.
///
/// With no XML parser, for the reason the rest of this project gives: bringing
/// one in to find two tags is the same excess as an HTTP client for a loopback
/// call. Pure, so the slicing is testable — and it is worth testing, because a
/// key one character too long fails with `403 Forbidden`, which reads like a
/// permission problem and not like an index one.
#[must_use]
pub fn between(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let shut = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&shut)? + start;
    Some(xml[start..end].to_owned())
}

/// Where the web interface listens, as Syncthing itself recorded it.
///
/// `$RONOCTUA_SYNCTHING_GUI` wins, for the throwaway instance the tests raise.
#[must_use]
pub fn address() -> Option<String> {
    if let Some(set) = std::env::var_os("RONOCTUA_SYNCTHING_GUI") {
        let set = set.to_string_lossy().into_owned();
        if !set.is_empty() {
            return Some(set);
        }
    }
    let text = configuration()?;
    // THE ADDRESS IS INSIDE `<gui>`, and `<address>` also appears inside every
    // `<device>` — many times, holding a peer's address. Taking the first
    // `<address>` in the file gives a REMOTE machine's address, and every call
    // then goes somewhere it has no business going.
    let gui = section(&text, "gui")?;
    between(&gui, "address")
}

/// One `<tag …> … </tag>` block, attributes and all.
///
/// Needed because `<gui>` carries attributes, so `between` — which looks for
/// the exact `<gui>` — would never find it.
#[must_use]
pub fn section(xml: &str, tag: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}"))?;
    let opened = xml[start..].find('>')? + start + 1;
    let shut = format!("</{tag}>");
    let end = xml[opened..].find(&shut)? + opened;
    Some(xml[opened..end].to_owned())
}

/// The API key, from the `config.xml`. Private on purpose.
fn key() -> Option<String> {
    let text = configuration()?;
    between(&section(&text, "gui")?, "apikey")
}

/// THE PATHS, AND THIS IS EVERY ONE OF THEM.
///
/// They used to be spelt where they were USED — scattered through `look`,
/// `errand_of` and `Scope::path` — while the module header above claimed this
/// was the only place that knew a `/rest/…` existed. It was not, and nothing
/// said so until `one-syncthing-boundary` was written and pointed at its own
/// crate, which is the first thing it found. The claim is true now, and the
/// check is what keeps it true.
///
/// Every machine but this one, stopped.
pub const PAUSE: &str = "/rest/system/pause";

/// The same, started again.
pub const RESUME: &str = "/rest/system/resume";

/// Syncthing restarting itself. No privilege: it does this to itself.
pub const RESTART: &str = "/rest/system/restart";

/// The cheapest thing that proves it is answering.
pub const PING: &str = "/rest/system/ping";

/// What a message says when the configuration cannot be read at all.
///
/// A CONSTANT rather than a sentence written twice: `refusal` classifies by
/// the text of the error, so the two would have to be kept in step by hand —
/// and the file's own name would then be spelt outside this module, which is
/// exactly what the boundary is about.
pub const NOT_READABLE: &str = "the configuration is not readable";

/// Everything the service says about itself.
///
/// # Errors
///
/// As [`call`].
pub fn status() -> anyhow::Result<String> {
    call("GET", "/rest/system/status", None)
}

/// Which version it is.
///
/// # Errors
///
/// As [`call`].
pub fn version() -> anyhow::Result<String> {
    call("GET", "/rest/system/version", None)
}

/// The whole configuration.
///
/// # Errors
///
/// As [`call`].
pub fn config() -> anyhow::Result<String> {
    call("GET", "/rest/config", None)
}

/// The live connection table.
///
/// # Errors
///
/// As [`call`].
pub fn connections() -> anyhow::Result<String> {
    call("GET", "/rest/system/connections", None)
}

/// How much of everything is here.
///
/// # Errors
///
/// As [`call`].
pub fn completion() -> anyhow::Result<String> {
    call("GET", "/rest/db/completion", None)
}

/// Whatever it is complaining about.
///
/// # Errors
///
/// As [`call`].
pub fn errors() -> anyhow::Result<String> {
    call("GET", "/rest/system/error", None)
}

/// Machines asking to be let in.
///
/// # Errors
///
/// As [`call`].
pub fn pending_devices() -> anyhow::Result<String> {
    call("GET", "/rest/cluster/pending/devices", None)
}

/// Folders somebody offered.
///
/// # Errors
///
/// As [`call`].
pub fn pending_folders() -> anyhow::Result<String> {
    call("GET", "/rest/cluster/pending/folders", None)
}

/// One folder's state and counts.
///
/// # Errors
///
/// As [`call`].
pub fn folder_status(folder: &str) -> anyhow::Result<String> {
    call(
        "GET",
        &format!("/rest/db/status?folder={}", crate::escaped(folder)),
        None,
    )
}

/// One folder's configuration object.
///
/// # Errors
///
/// As [`call`].
pub fn one_folder(folder: &str) -> anyhow::Result<String> {
    call("GET", &a_folder(folder), None)
}

/// The template a new folder or machine starts from.
///
/// # Errors
///
/// As [`call`], or the scope is not one that has a template.
pub fn defaults(scope: crate::options::Scope) -> anyhow::Result<String> {
    call("GET", &object_of(scope, None), None)
}

/// Where one configuration object lives.
///
/// `at` is the folder's or machine's id, and is ignored by the scopes that do
/// not have one.
#[must_use]
pub fn object_of(scope: crate::options::Scope, at: Option<&str>) -> String {
    use crate::options::Scope;
    match scope {
        Scope::Folder => a_folder(at.unwrap_or("")),
        Scope::Device => a_device(at.unwrap_or("")),
        Scope::Global => "/rest/config/options".to_owned(),
        Scope::Interface => "/rest/config/gui".to_owned(),
        Scope::FolderDefaults => "/rest/config/defaults/folder".to_owned(),
        Scope::DeviceDefaults => "/rest/config/defaults/device".to_owned(),
    }
}

/// One folder's configuration.
#[must_use]
pub fn a_folder(folder: &str) -> String {
    format!("/rest/config/folders/{}", crate::escaped(folder))
}

/// One machine's configuration.
#[must_use]
pub fn a_device(device: &str) -> String {
    format!("/rest/config/devices/{}", crate::escaped(device))
}

/// Stop talking to one machine.
#[must_use]
pub fn pause_one(device: &str) -> String {
    format!("{PAUSE}?device={}", crate::escaped(device))
}

/// Start again.
#[must_use]
pub fn resume_one(device: &str) -> String {
    format!("{RESUME}?device={}", crate::escaped(device))
}

/// Look at one folder's disk now.
#[must_use]
pub fn scan(folder: &str) -> String {
    format!("/rest/db/scan?folder={}", crate::escaped(folder))
}

/// Send this machine's copy of a folder out as the truth.
#[must_use]
pub fn override_it(folder: &str) -> String {
    format!("/rest/db/override?folder={}", crate::escaped(folder))
}

/// Throw this machine's changes to a folder away.
#[must_use]
pub fn revert(folder: &str) -> String {
    format!("/rest/db/revert?folder={}", crate::escaped(folder))
}

/// Forget that a machine asked.
#[must_use]
pub fn pending_device(device: &str) -> String {
    format!(
        "/rest/cluster/pending/devices?device={}",
        crate::escaped(device)
    )
}

/// Forget that a machine offered a folder.
///
/// BOTH ARE NAMED, because a folder is offered by one or more machines and the
/// offer is keyed by machine: rejecting by folder alone would say no to all of
/// them at once.
#[must_use]
pub fn pending_folder(folder: &str, from: &str) -> String {
    format!(
        "/rest/cluster/pending/folders?folder={}&device={}",
        crate::escaped(folder),
        crate::escaped(from)
    )
}

/// One call to the API, with the ordinary deadline.
///
/// # Errors
///
/// There is no configuration to read the key out of, nothing is listening, the
/// service did not answer in time, or it refused — and a refusal carries
/// Syncthing's own words, which are worth putting on the screen.
pub fn call(method: &str, path: &str, body: Option<&str>) -> anyhow::Result<String> {
    ask(method, path, body, ronoctua_http::PATIENCE)?.ok()
}

/// The same, but the status comes back instead of becoming an error.
///
/// # Errors
///
/// As [`call`], minus the refusal: a 4xx is an answer here.
pub fn answer(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> anyhow::Result<ronoctua_http::Answer> {
    ask(method, path, body, ronoctua_http::PATIENCE)
}

/// The long poll. It BLOCKS until something happens or [`HOLD`] runs out.
///
/// # Errors
///
/// As [`call`].
pub fn poll(since: u64) -> anyhow::Result<String> {
    let path = format!("/rest/events?since={since}&timeout={}", HOLD.as_secs());
    ask("GET", &path, None, HOLD_PATIENCE)?.ok()
}

/// Everything the two above share.
fn ask(
    method: &str,
    path: &str,
    body: Option<&str>,
    patience: Duration,
) -> anyhow::Result<ronoctua_http::Answer> {
    let key = key().ok_or_else(|| anyhow::anyhow!("{NOT_READABLE}: no API key"))?;
    let host = address().ok_or_else(|| anyhow::anyhow!("{NOT_READABLE}: no address"))?;
    let at: std::net::SocketAddr = host
        .parse()
        .map_err(|e| anyhow::anyhow!("the configured address {host} is not one: {e}"))?;
    let headers = [("X-API-Key", key.as_str())];
    let mut about = ronoctua_http::Call::new(method, path, &host)
        .with_headers(&headers)
        .waiting(patience);
    if let Some(body) = body {
        about = about.with_body(body);
    }
    ronoctua_http::over_tcp(at, &about)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_comes_from_between_the_tags() {
        // With no XML parser: the test locks the slice down, which is where an
        // index error would go unnoticed — a key with one character too many
        // fails with "403 Forbidden", which looks like a permission problem
        // and not like a slicing one.
        let xml = "<configuration><gui><apikey>abc123</apikey></gui></configuration>";
        assert_eq!(between(xml, "apikey").as_deref(), Some("abc123"));
        assert_eq!(between(xml, "nothing"), None);
    }

    #[test]
    fn a_section_with_attributes_is_still_found() {
        // `<gui>` carries three attributes in every real file, so looking for
        // the exact string `<gui>` finds nothing at all.
        let xml = r#"<configuration><gui enabled="true" tls="false"><address>1.2.3.4:1</address></gui></configuration>"#;
        assert_eq!(between(xml, "gui"), None, "the bare tag does not exist");
        let gui = section(xml, "gui").expect("the section is found by its opening");
        assert_eq!(between(&gui, "address").as_deref(), Some("1.2.3.4:1"));
    }

    #[test]
    fn the_address_is_the_interfaces_and_never_a_peers() {
        // THE DEFECT THIS PREVENTS: `<address>` appears inside every
        // `<device>`, holding a REMOTE machine's address. Taking the first one
        // in the file points every call at somebody else's computer.
        let xml = concat!(
            "<configuration>",
            r#"<device id="AAA"><address>tcp://198.51.100.7:22000</address></device>"#,
            r#"<gui enabled="true"><address>127.0.0.1:8384</address><apikey>k</apikey></gui>"#,
            "</configuration>",
        );
        assert_eq!(
            between(xml, "address").as_deref(),
            Some("tcp://198.51.100.7:22000"),
            "the naive read finds the peer, which is the point"
        );
        let gui = section(xml, "gui").unwrap();
        assert_eq!(between(&gui, "address").as_deref(), Some("127.0.0.1:8384"));
    }

    #[test]
    fn the_poll_waits_longer_than_it_asks_to_be_held() {
        // If it did not, the deadline would fire before Syncthing answered and
        // EVERY poll would read as a service that stopped answering — a view
        // permanently saying the daemon is down while it is up and idle.
        assert!(
            HOLD_PATIENCE > HOLD,
            "the deadline must outlast the hold, or an idle service reads as a dead one"
        );
    }

    #[test]
    fn the_directory_follows_the_environment_and_names_nobody() {
        // INVARIANT 16: the owner's name lives in `nix/modules/base.nix` and
        // nowhere else. This module used to fall back to a literal home
        // directory, and the doc comment above still quotes it — which is why
        // the comments come off first. A check that reads its own prose passes
        // on the words instead of on the code, and this repository has already
        // shipped one that did.
        let source = include_str!("rest.rs");
        let code: String = source
            .split("#[cfg(test)]")
            .next()
            .expect("there is code before the tests")
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("/home/"),
            "a home directory is written into the code"
        );
    }
}
