//! The boundary with Atuin — the shell history, and whether it leaves here.
//!
//! # The decision this reverses, and who made it
//!
//! `nix/modules/home.nix` carried, in words:
//!
//! ```text
//! # NO SYNC. Syncing shell history to a third-party server would put
//! # every command — including the ones with a token in argv — outside
//! # this machine. Section 9 does not allow that quietly.
//! auto_sync = false;
//! ```
//!
//! That was right about the risk and it decided for the person. The owner
//! asked for the opposite, and two things the sentence did not weigh are why
//! it can be offered without the risk changing:
//!
//!   * Atuin's sync is END TO END ENCRYPTED. The server holds ciphertext, and
//!     the key is `atuin key` — it never crosses. "A third-party server" is
//!     then a store of bytes nobody there can read.
//!   * The server can be YOURS. `services.atuin` is in the pinned nixpkgs, and
//!     `nix/modules/atuin.nix` offers it — so "third party" need not describe
//!     the arrangement at all.
//!
//! What has NOT changed is the command with a token in its `argv`. That is why
//! `secrets_filter` is a drawn row of the view rather than a default nobody
//! reads: the thing that decides what is never recorded belongs on the screen
//! next to the switch that decides where it goes.
//!
//! **It is still off by default.** The flake is the floor; the person turns it
//! on, and the screen says what leaves the machine.

pub mod cli;
pub mod config;
pub mod options;
pub mod prefs;

use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

/// How often the thread asks, at rest.
///
/// Thirty seconds. `atuin status` is a process, not a socket, so this is the
/// one place in this tree where a beat costs a fork — and nothing here changes
/// between one minute and the next unless somebody pressed a key, which the
/// order channel answers immediately.
pub const BEAT: Duration = Duration::from_secs(30);

/// What the view draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Is `atuin` on this machine at all?
    pub present: bool,
    /// Is there an account on a server?
    pub logged_in: bool,
    /// Is `auto_sync` on?
    pub syncing: bool,
    /// The server, as Atuin prints it.
    pub address: String,
    /// The account's name.
    pub username: String,
    /// How often it syncs by itself.
    pub frequency: String,
    /// When it last did.
    pub last: String,
    /// How many entries the history holds.
    pub entries: u64,
    /// Every setting the file says, by `section.key`.
    pub settings: std::collections::BTreeMap<String, String>,
    /// What went wrong last, in Atuin's own words.
    pub trouble: Option<String>,
    /// Which modeline mark this machine draws — OURS, not Atuin's.
    ///
    /// It travels in the snapshot so a door draws it from the same reading as
    /// everything else. A view that read the file itself would be a second
    /// reader with its own idea of when it is stale.
    pub mark: prefs::Mark,
}

impl Snapshot {
    /// Is the history actually crossing the network?
    ///
    /// What the modeline's mark asks. Logged-in-but-not-syncing and
    /// not-logged-in are different sentences in the view and one fact here.
    #[must_use]
    pub const fn crossing(&self) -> bool {
        self.present && self.logged_in && self.syncing
    }
}

/// What a door can ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Sync now.
    Sync,
    /// Log in to a server.
    LogIn {
        /// Where it is.
        address: String,
        /// Who to be.
        username: String,
        /// What they typed.
        password: String,
        /// The key from the first machine, or empty on the first machine.
        key: String,
    },
    /// Forget the account and the key.
    LogOut,
    /// Write one setting into `config.toml`.
    Set {
        /// Which section, if any.
        section: Option<String>,
        /// Which key.
        key: String,
        /// What to write, already in TOML's spelling.
        value: String,
    },
    /// Turn the modeline's mark.
    Mark(prefs::Mark),
}

/// What a door hears back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A whole fresh reading.
    Snapshot(Box<Snapshot>),
    /// An order finished.
    Done {
        /// What was asked for.
        order: Order,
        /// What went wrong, when something did.
        refused: Option<String>,
    },
    /// The encryption key, which was asked for and is shown once.
    Key(String),
}

/// The handle a door keeps.
pub struct Hand {
    orders: mpsc::Sender<Order>,
}

impl Hand {
    /// Asks for something.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::debug!("the atuin thread is gone; the order was dropped");
        }
    }
}

/// What the MODELINE needs, from files alone — no process, no thread.
///
/// # Why this exists beside [`look`]
///
/// Because the mark has to be true on a machine where nobody has opened the
/// view, and [`look`] costs a `fork` (`atuin status` is a program). Raising the
/// watching thread at start-up so that a glyph could be right would make every
/// machine in this distro pay one every thirty seconds, for ever, for a mark
/// that most of them never draw.
///
/// Everything the mark asks is already on disk:
///
/// * **logged in** — `atuin login` writes a session token and `atuin logout`
///   removes it. The file is `$XDG_DATA_HOME/atuin/session`; measured against
///   the 18.x on this machine, whose `key_path` default sits beside it.
/// * **syncing** — `auto_sync` in `config.toml`, which this crate already
///   reads and writes.
/// * **the ring** — ours, in `atuin.json`.
/// * **present** — the binary is a path baked at build time
///   ([`cli::ATUIN`]); asking whether it EXISTS is a `stat`, not a `fork`.
///
/// What it deliberately does NOT fill is the account's name, the address, the
/// last sync and the entry count: those are what `atuin status` answers, and a
/// screen that showed a blank where a name goes would be worse than a screen
/// that waits. The view opens the thread and gets all of it.
#[must_use]
pub fn at_rest() -> Snapshot {
    let settings = config::path().map(|p| config::read(&p)).unwrap_or_default();
    Snapshot {
        present: std::path::Path::new(cli::ATUIN).exists(),
        logged_in: session_path().is_some_and(|p| p.exists()),
        syncing: auto_sync(&settings),
        settings,
        mark: prefs::read().mark,
        ..Snapshot::default()
    }
}

/// Where Atuin keeps the token that says this machine has an account.
fn session_path() -> Option<std::path::PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME").map_or_else(
        || std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")),
        |d| Some(std::path::PathBuf::from(d)),
    )?;
    Some(data.join("atuin").join("session"))
}

/// `auto_sync`, as the file says or as Atuin would decide.
///
/// ATUIN'S OWN DEFAULT IS TRUE when the file does not say. Reading an absent
/// line as `false` would draw a switch that is off beside a history that is
/// syncing.
fn auto_sync(settings: &std::collections::BTreeMap<String, String>) -> bool {
    settings
        .get("auto_sync")
        .is_none_or(|v| v.trim().eq_ignore_ascii_case("true"))
}

/// Reads everything, once.
#[must_use]
pub fn look() -> Snapshot {
    let status = cli::status();
    let present = status.is_ok();
    let status = status.unwrap_or_default();
    let settings = config::path().map(|p| config::read(&p)).unwrap_or_default();
    let syncing = auto_sync(&settings);
    Snapshot {
        present,
        logged_in: status.logged_in,
        syncing,
        address: status.address,
        username: status.username,
        frequency: status.frequency,
        last: status.last,
        entries: cli::how_many().unwrap_or(0),
        settings,
        trouble: None,
        mark: prefs::read().mark,
    }
}

/// Raises the thread.
///
/// # Errors
///
/// The thread could not be started.
pub fn watch<F>(on_event: F) -> Result<Hand>
where
    F: Fn(Event) + Send + 'static,
{
    let (orders, inbox) = mpsc::channel::<Order>();
    std::thread::Builder::new()
        // FIFTEEN BYTES is Linux's whole room for a thread's name.
        .name("atuin".to_owned())
        .spawn(move || {
            let mut last = Snapshot::default();
            on_event(Event::Snapshot(Box::new(look())));
            loop {
                match inbox.recv_timeout(BEAT) {
                    Ok(order) => {
                        let refused = carry_out(&order, &on_event).err();
                        on_event(Event::Done {
                            order,
                            refused: refused.as_ref().map(ToString::to_string),
                        });
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                let fresh = look();
                if fresh != last {
                    last = fresh.clone();
                    on_event(Event::Snapshot(Box::new(fresh)));
                }
            }
            tracing::debug!("the atuin thread ended with its last sender");
        })
        .map_err(|e| anyhow::anyhow!("could not start the atuin thread: {e}"))?;
    Ok(Hand { orders })
}

/// Does one order.
fn carry_out<F: Fn(Event)>(order: &Order, on_event: &F) -> Result<()> {
    match order {
        Order::Sync => cli::sync(),
        Order::LogOut => cli::logout(),
        Order::LogIn {
            address,
            username,
            password,
            key,
        } => {
            cli::login(address, username, password, key)?;
            // AND THE ADDRESS IS WRITTEN DOWN, because `atuin login` takes it
            // from the environment and the next process will not have it.
            let Some(path) = config::path() else {
                return Ok(());
            };
            config::put(&path, (None, "sync_address"), &format!("\"{address}\""))
        }
        Order::Set {
            section,
            key,
            value,
        } => {
            let path = config::path()
                .ok_or_else(|| anyhow::anyhow!("there is nowhere to write atuin's settings"))?;
            config::put(&path, (section.as_deref(), key), value)
        }
        Order::Mark(mark) => {
            prefs::write(&prefs::Preferences { mark: *mark }).map_err(|e| anyhow::anyhow!("{e}"))
        }
    }
    .inspect(|()| {
        if matches!(order, Order::LogOut) {
            on_event(Event::Key(String::new()));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_needs_all_three() {
        // PRESENT, LOGGED IN AND SYNCING. The view tells the three apart in
        // words; the mark asks this one question.
        let whole = Snapshot {
            present: true,
            logged_in: true,
            syncing: true,
            ..Snapshot::default()
        };
        assert!(whole.crossing());
        for take in 0..3 {
            let mut one = whole.clone();
            match take {
                0 => one.present = false,
                1 => one.logged_in = false,
                _ => one.syncing = false,
            }
            assert!(!one.crossing(), "it claimed to sync with {take} missing");
        }
    }

    #[test]
    fn a_machine_with_no_atuin_is_not_a_broken_one() {
        let nothing = Snapshot::default();
        assert!(!nothing.present);
        assert!(!nothing.crossing());
        assert!(nothing.trouble.is_none());
    }

    #[test]
    fn an_absent_auto_sync_reads_as_atuins_own_default() {
        // ATUIN'S DEFAULT IS TRUE. Reading an absent line as `false` would
        // draw a switch that is off beside a history that is syncing — and the
        // person would turn it "on" and watch nothing change.
        let mut settings = std::collections::BTreeMap::new();
        assert!(
            settings
                .get("auto_sync")
                .is_none_or(|v: &String| v.trim().eq_ignore_ascii_case("true"))
        );
        settings.insert("auto_sync".to_owned(), "false".to_owned());
        assert!(
            !settings
                .get("auto_sync")
                .is_none_or(|v: &String| v.trim().eq_ignore_ascii_case("true"))
        );
    }

    #[test]
    fn the_beat_is_slow_because_it_costs_a_fork() {
        // `atuin status` IS A PROCESS, not a socket — the one place in this
        // tree where a beat costs a fork. A key press does not wait for it:
        // the order channel answers at once.
        assert!(BEAT >= Duration::from_secs(15));
    }
}
