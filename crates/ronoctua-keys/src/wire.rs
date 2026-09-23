//! The socket between the daemon and the footer: JSON, one line per message.
//!
//! # Why a socket and not the compositor
//!
//! In Independent mode the footer never takes the Wayland keyboard: the
//! compositor gives an exclusive layer EVERY seat's keyboard at once
//! (`layer_shell.c:130` in the pinned source), so opening the leader from one
//! keyboard would silence all the others. Instead the daemon — which already
//! has every keyboard — says which one tapped its leader, and while the footer
//! holds that keyboard it DIVERTS it: its keys come here, and nothing goes to
//! its virtual twin. The other keyboards keep typing into their windows.
//!
//! # What crosses, and what never does
//!
//! A key crosses only while the footer holds that keyboard, and only to the
//! session's user (the socket is `0600` and theirs). Nothing is logged about a
//! key, here or in the daemon.

use serde::{Deserialize, Serialize};

/// Where the daemon listens — the unit's `RuntimeDirectory`.
pub const SOCKET: &str = "/run/ronoctua-keys/keys.sock";

/// What the daemon says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "said")]
pub enum FromDaemon {
    /// The units it holds right now — sent on connect and on every change.
    Units {
        /// By identity, with what the kernel calls each.
        units: Vec<Held>,
    },
    /// This unit's leader was tapped.
    Leader {
        /// Its identity.
        unit: String,
    },
    /// A key of a diverted unit, after its remaps.
    Key {
        /// Its identity.
        unit: String,
        /// The kernel's key code.
        code: u16,
        /// 1 press, 0 release, 2 autorepeat.
        value: i32,
    },
}

/// One unit the daemon holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// [`ronoctua_keyboards::Unit::identity`].
    pub identity: String,
    /// What the kernel calls the physical unit.
    pub name: String,
    /// Its model, `vendor:product`.
    pub model: String,
}

/// What the footer says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "asked")]
pub enum ToDaemon {
    /// Send me this unit's keys, and nothing to its twin.
    Divert {
        /// Its identity.
        unit: String,
    },
    /// Give this unit back to its twin.
    Release {
        /// Its identity.
        unit: String,
    },
}

/// The name a unit's virtual twin carries — what the compositor lists.
#[must_use]
pub fn twin_name(identity: &str) -> String {
    format!("ronoctua keyboard {identity}")
}

/// The compositor's identifier for a unit's twin: `vendor:product:name`, the
/// name's spaces as underscores. Vendor and product are zero, which is what
/// keeps the twins out of [`ronoctua_keyboards::seen`]'s list of keyboards.
#[must_use]
pub fn twin_identifier(identity: &str) -> String {
    format!("0:0:{}", twin_name(identity).replace(' ', "_"))
}

/// The unit a twin's compositor identifier names, if it is one.
#[must_use]
pub fn unit_of_twin(identifier: &str) -> Option<&str> {
    identifier.strip_prefix("0:0:ronoctua_keyboard_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_one_line_of_json() {
        let m = FromDaemon::Key {
            unit: "0a1b2c3d".into(),
            code: 30,
            value: 1,
        };
        let line = serde_json::to_string(&m).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<FromDaemon>(&line).unwrap(), m);
        let ask: ToDaemon = serde_json::from_str(r#"{"asked":"divert","unit":"x"}"#).unwrap();
        assert_eq!(ask, ToDaemon::Divert { unit: "x".into() });
    }

    #[test]
    fn a_twin_is_found_by_its_identifier() {
        let id = twin_identifier("0a1b2c3d");
        assert_eq!(id, "0:0:ronoctua_keyboard_0a1b2c3d");
        assert_eq!(unit_of_twin(&id), Some("0a1b2c3d"));
        assert_eq!(unit_of_twin("1:1:AT_Translated_Set_2_keyboard"), None);
    }
}
