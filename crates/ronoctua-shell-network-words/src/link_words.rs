//! THE WAY OUT AS THE SHELL SAYS IT — what the footer, which keeps
//! NetworkManager's reading and the two tunnels' for the session, and the
//! network view (`ronoctua-shell-network`) both say and read: a tunnel's
//! refusal in a sentence, the hotspot's password file, the program a login
//! page is opened with, and the words a modeline mark opens the view on its
//! row with.
//!
//! A leaf under both, on no other shell crate: the footer names no feature,
//! and a line of the view recompiles the view and the binary alone.

/// `xdg-open`, baked at build time — what opens a tailnet's login page.
///
/// A LOGIN IS A PAGE, and this system has no browser of its own: the address
/// the daemon hands back goes to whatever the person's own desktop opens
/// links with, which is what `xdg-open` answers.
pub const OPEN: &str = match option_env!("RONOCTUA_XDG_OPEN") {
    Some(p) => p,
    None => "xdg-open",
};

/// What the message line says when a tunnel's daemon refused an order —
/// by name, and with the line Tor quoted when it quoted one: that is the
/// person's own text given back, and the one thing that says which bridge was
/// pasted wrong.
#[must_use]
pub fn tunnel_refused(r: &ronoctua_tunnel::Refused, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_tunnel::Refusal;
    tracing::info!(why = ?r.why, words = %r.words, "a tunnel refused");
    if let (Refusal::ConfigRefused, Some(line)) = (r.why, r.line.as_deref()) {
        return texts.t_with(key::ECHO_TUNNEL_REFUSED_LINE, &[("line", line)]);
    }
    let reason = texts.t(match r.why {
        Refusal::NoDaemon => key::TUNNEL_REFUSED_NO_DAEMON,
        Refusal::NotAuthorized => key::TUNNEL_REFUSED_AUTHORITY,
        Refusal::UnitFailed => key::TUNNEL_REFUSED_UNIT,
        Refusal::NotWritten => key::TUNNEL_REFUSED_NOT_WRITTEN,
        Refusal::ConfigRefused => key::TUNNEL_REFUSED_CONFIG,
        Refusal::DaemonRefused => key::TUNNEL_REFUSED_DAEMON,
        Refusal::Other => key::TUNNEL_REFUSED_OTHER,
    });
    texts.t_with(key::ECHO_NETWORK_REFUSED, &[("why", &reason)])
}

/// The hotspot's password, kept where BOTH doors read it.
///
/// # Why a file, and why this one
///
/// The password lived on the view of the door that turned the hotspot on,
/// so the other door said "this machine is the network now" without it, and
/// reopening the footer's view lost it. Asking NetworkManager for it
/// (`GetSecrets`) can need an authorisation for a system profile, and
/// nothing the system does on its own may ask. So the door that starts the
/// hotspot writes the password HERE — `$XDG_RUNTIME_DIR`, a tmpfs that is the
/// person's alone, in a file born `0600` (`create_new` + mode, never a
/// `chmod` after) — and the door that sees it stop takes it away. It is the
/// shape the radio's standing question has (`bluez_words::standing`).
pub mod sharing {
    /// The file's name, after the instance.
    const LEAF: &str = "hotspot.json";

    /// What the file holds.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Kept {
        /// The hotspot's name.
        pub network: String,
        /// Its password.
        pub password: String,
    }

    /// `$XDG_RUNTIME_DIR/<name>-hotspot.json` — per instance and not per
    /// display: the hotspot is the machine's, whichever screen started it.
    #[must_use]
    pub fn path() -> Option<std::path::PathBuf> {
        ronoctua_xdg::runtime_file(LEAF)
    }

    /// Writes the hotspot that is up, whole and owner-only: a draft born
    /// `0600`, renamed over the file.
    ///
    /// # Errors
    ///
    /// No runtime directory, or the draft could not be written.
    pub fn keep(kept: &Kept) -> std::io::Result<()> {
        let path = path().ok_or_else(|| std::io::Error::other("no runtime directory"))?;
        keep_at(&path, kept)
    }

    /// [`keep`], at a path the caller gives — the tests' own.
    ///
    /// # Errors
    ///
    /// The draft could not be written or renamed.
    pub fn keep_at(path: &std::path::Path, kept: &Kept) -> std::io::Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let draft = path.with_extension(format!("json.{}", std::process::id()));
        // A DRAFT LEFT BY A DOOR THAT DIED is this process's name only if
        // the PID came round again; `create_new` refuses it, so it goes first.
        let _ = std::fs::remove_file(&draft);
        let text = serde_json::to_string(kept).map_err(std::io::Error::other)?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&draft)?;
        f.write_all(text.as_bytes())?;
        drop(f);
        std::fs::rename(&draft, path)
    }

    /// The hotspot that is up, as the file says it.
    #[must_use]
    pub fn read() -> Option<Kept> {
        read_at(&path()?)
    }

    /// [`read`], at a path the caller gives.
    #[must_use]
    pub fn read_at(path: &std::path::Path) -> Option<Kept> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Follows the hotspot over one reading of the daemon: when a hotspot
    /// that was up is NOT up any more, the file goes. `was` is the caller's
    /// memory of the last reading — the footer keeps one whether or not its
    /// view is open, so the password never outlives the hotspot. A reading
    /// that has never seen it up deletes nothing: the door that has just
    /// started it, a moment before the daemon says so, keeps its file.
    pub fn follow(was: &mut bool, net: &ronoctua_network::Snapshot) -> bool {
        let serving = serving(net);
        if *was && !serving {
            forget();
        }
        *was = serving;
        serving
    }

    /// Whether this machine is serving a hotspot, by one reading — what a
    /// view shows, with no memory and nothing taken away: the door that keeps
    /// the memory follows it ([`follow`]).
    #[must_use]
    pub fn serving(net: &ronoctua_network::Snapshot) -> bool {
        net.links.iter().any(|l| l.serving)
    }

    /// Takes the file away — the hotspot is down. Nothing there is not an
    /// error.
    pub fn forget() {
        if let Some(path) = path() {
            forget_at(&path);
        }
    }

    /// [`forget`], at a path the caller gives.
    pub fn forget_at(path: &std::path::Path) {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "the hotspot's password could not be taken away"),
        }
    }
}

/// The argument `network-open` takes from the tailnet's modeline mark — the
/// view opens on the tailnet's section: a mark is a door onto the thing it
/// is about (the view's `network_region::opening_on`).
pub const TAILNET_OPENING: &str = "tailnet";

/// The argument `network-open` takes from Tor's mark — Tor's section.
pub const TOR_OPENING: &str = "tor";

/// What starts the argument `network-open` takes from a tunnel's mark,
/// before the connection's object path ([`tunnel_opening`]).
const TUNNEL_OPENING: &str = "tunnel ";

/// The argument `network-open` takes from a tunnel's mark: the view opens on
/// that connection's row, `at` its object path.
#[must_use]
pub fn tunnel_opening(at: &str) -> String {
    format!("{TUNNEL_OPENING}{at}")
}

/// The connection a [`tunnel_opening`] names — `None` for an argument that
/// is none.
#[must_use]
pub fn tunnel_opened(argument: &str) -> Option<&str> {
    argument.strip_prefix(TUNNEL_OPENING)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the hotspot's password file is born readable by anybody but
    /// its owner, is not read back whole, or is not taken away when the
    /// hotspot that was up stops.
    #[test]
    fn the_hotspot_password_file_is_the_owners_and_goes_with_the_hotspot() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ronoctua-hotspot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hotspot.json");
        let kept = sharing::Kept {
            network: "noctua".to_owned(),
            password: "abc12345".to_owned(),
        };
        sharing::keep_at(&path, &kept).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "born {mode:o}");
        assert_eq!(sharing::read_at(&path), Some(kept.clone()));
        // WRITTEN AGAIN over the old one, still owner-only.
        sharing::keep_at(&path, &kept).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        sharing::forget_at(&path);
        assert_eq!(sharing::read_at(&path), None);
        sharing::forget_at(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a tunnel's refusal carries the daemon's English after its
    /// colon, or the line Tor quoted is not given back.
    #[test]
    fn a_tunnels_refusal_is_said_by_its_name() {
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let line = tunnel_refused(
            &ronoctua_tunnel::Refused {
                why: ronoctua_tunnel::Refusal::ConfigRefused,
                line: Some("Brige obfs4".to_owned()),
                words: "553 Unrecognized option".to_owned(),
            },
            &t,
        );
        assert!(
            line.contains("Brige obfs4") && !line.contains("Unrecognized"),
            "{line}"
        );
        let unit = tunnel_refused(
            &ronoctua_tunnel::Refused {
                why: ronoctua_tunnel::Refusal::UnitFailed,
                line: None,
                words: "Job failed".to_owned(),
            },
            &t,
        );
        assert!(!unit.contains("Job"), "{unit}");
    }

    /// FAILS IF: the connection a tunnel's mark names comes back as another
    /// one, or a section's word is taken for a connection.
    #[test]
    fn a_tunnels_opening_names_its_connection() {
        let at = "/org/freedesktop/NetworkManager/ActiveConnection/7";
        assert_eq!(tunnel_opened(&tunnel_opening(at)), Some(at));
        assert_eq!(tunnel_opened(TAILNET_OPENING), None);
        assert_eq!(tunnel_opened(TOR_OPENING), None);
    }
}
