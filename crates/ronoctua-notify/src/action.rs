//! What a notification lets you DO — section 4.2.
//!
//! # Two origins, and they must not be flattened together
//!
//! An application on the bus sends `actions` as a flat list of pairs — an id
//! and a label already in some language it chose. We cannot translate that and
//! we must not pretend to: the only honest thing is to draw the text it sent
//! and hand the id back through `ActionInvoked`.
//!
//! Ours are the opposite. They are declared in `assets/notifications.nix`, the
//! label is a text KEY (invariant 8), and what they do is a program or a
//! command of ours — never a round trip through a bus nobody is listening on.
//!
//! [`Label`] and [`Doing`] are that difference made impossible to lose.

/// How the action's name reaches the screen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Label {
    /// A text key — ours, and translated (invariant 8).
    Key(String),
    /// Raw text, as an application on the bus sent it.
    ///
    /// It is not translated and it must not be: we do not know what it says.
    Text(String),
}

/// A program an action of ours may start.
///
/// # Why a closed set, and not a path
///
/// Because a store path in this crate is a reference this crate's closure
/// carries, and because `assets/notifications.nix` would then be naming
/// binaries — which is how a table stops being data. The shell resolves each of
/// these from the environment it already receives (`RONOCTUA_*`), which is the
/// same door the file manager's "open with" uses.
///
/// A program that is not installed makes its action DISAPPEAR from the
/// notification, rather than fail when clicked. An action that does nothing is
/// worse than an action that is not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Program {
    /// `xdg-open` — whatever claims the type.
    Open,
    /// `swappy` — the screenshot editor. Full profile only.
    Edit,
}

impl Program {
    /// The name the generated table carries.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Edit => "edit",
        }
    }

    /// From the name in the table.
    #[must_use]
    pub fn by_id(s: &str) -> Option<Self> {
        Some(match s {
            "open" => Self::Open,
            "edit" => Self::Edit,
            _ => return None,
        })
    }

    /// Every one of them, for the tests and the generator.
    pub const ALL: [Self; 2] = [Self::Open, Self::Edit];
}

/// One argument of a program an action starts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Arg {
    /// Written in the table, as it is.
    Literal(String),
    /// The FILE the notification is about.
    ///
    /// It comes from the `file://` URL the sender put in the icon field — which
    /// is exactly what the footer does with a capture's path. Without a file,
    /// the action is not offered.
    File,
    /// The DIRECTORY that file is in.
    Folder,
}

/// What the action does.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Doing {
    /// Hand the id back to the sender through `ActionInvoked`.
    ///
    /// The only thing that can be done with a foreign app's action, and the
    /// thing it is waiting for.
    Invoke,
    /// Run a command of OURS, by id, from `ronoctua_cmd`'s registry.
    Command(String),
    /// Start a program with the notification's file.
    Run {
        /// Which program.
        program: Program,
        /// Its arguments, in order.
        args: Vec<Arg>,
    },
    /// Put the notification's file path on the clipboard.
    ///
    /// Its own variant and not a `Run`: putting text on the clipboard is one
    /// call of the shell's own, from either door, and a program started for
    /// it would be a process to do what that call does.
    CopyPath,
    /// Put the notification's file ITSELF on the clipboard, as an image —
    /// what a capture is taken for more often than not: to paste it.
    CopyImage,
}

/// One thing a notification lets you do.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Action {
    /// Stable id. For ours it is the kebab id from the table; for a foreign
    /// app's it is whatever the sender chose, and it goes back untouched.
    pub id: String,
    /// How the name is drawn.
    pub label: Label,
    /// What it does.
    pub doing: Doing,
}

impl Action {
    /// Reads the flat `(id, label)` list the protocol sends.
    ///
    /// The specification's list is pairs laid end to end. An odd length is a
    /// malformed message: the tail is dropped rather than paired with nothing,
    /// which would put an id where a label goes.
    ///
    /// `default` is the specification's reserved id for "the notification
    /// itself was clicked"; it has no button and is left out.
    #[must_use]
    pub fn from_protocol(flat: &[String]) -> Vec<Self> {
        flat.chunks_exact(2)
            .filter(|p| p[0] != "default")
            .map(|p| Self {
                id: p[0].clone(),
                label: Label::Text(p[1].clone()),
                doing: Doing::Invoke,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_protocols_flat_list_becomes_pairs() {
        let flat = ["open".to_owned(), "Open".to_owned(), "no".to_owned()];
        let out = Action::from_protocol(&flat);
        // The odd tail is DROPPED, not paired with nothing: an id drawn where a
        // label goes is a button that says `no`.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "open");
        assert_eq!(out[0].label, Label::Text("Open".to_owned()));
        assert_eq!(out[0].doing, Doing::Invoke);
    }

    #[test]
    fn the_reserved_default_action_is_not_a_button() {
        let flat = [
            "default".to_owned(),
            "".to_owned(),
            "reply".to_owned(),
            "Reply".to_owned(),
        ];
        let out = Action::from_protocol(&flat);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "reply");
    }

    #[test]
    fn a_programs_name_goes_and_comes_back() {
        for p in Program::ALL {
            assert_eq!(Program::by_id(p.id()), Some(p));
        }
        assert_eq!(Program::by_id("gimp"), None);
    }
}
