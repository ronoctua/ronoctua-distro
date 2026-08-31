//! Registry of named commands — section 13.
//!
//! "Every piece of functionality is a named, addressable command, not just a
//! key." This crate is where that name exists.
//!
//! # Why it is a crate of its own, and not part of the keymap
//!
//! `ronoctua-keymap` resolves the **key tree**; the command registry is
//! something else. What PRODUCES commands is the rest of the system —
//! `notify`, `term`, `runtime`, every buffer. What CONSUMES them are the keymap,
//! the minibuffer's `M-x` and the future IPC surface. If the ids were born inside
//! the `shell`, section 13's property would erode silently: a new shortcut
//! would be added with no corresponding command, and nothing would flag it.
//!
//! # Invariant 8
//!
//! A command does not carry UI text, it carries a **text key**. What
//! appears on screen comes from `ronoctua-i18n`, with `pt-BR` and `en-GB` complete.
//! That is why [`Command::label`] is a key and not a sentence.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

/// The text key prefix of every command.
///
/// The key and the id are the SAME thing written twice, and that is why one is
/// derivable from the other — see [`Failure::LabelDoesNotMatchId`].
pub const KEY_PREFIX_OF: &str = "cmd-";

/// A named command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// A stable id, in `kebab-case`. It is what the keymap and `M-x` reference.
    ///
    /// Genuinely stable: renaming an id breaks user keymaps and
    /// scripts. Treat it as part of the public interface.
    pub id: &'static str,
    /// A **key** of text, never the sentence. See invariant 8.
    ///
    /// It is always `cmd-<id>`, and the registry REJECTS anything else. Why
    /// then is it written out, if it is derivable? Because deriving it would produce
    /// a `String` built at run time, and invariant 8's guarantee
    /// comes from the key being a GENERATED CONSTANT: naming one that does not exist is
    /// a compile error. Written out, it is checked by the compiler; the rule
    /// below checks that it is the command's own key, and not somebody else's.
    pub label: &'static str,
}

/// Error while building the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Two commands with the same id.
    IdDuplicado(&'static str),
    /// An id outside the `kebab-case` format.
    IdMalFormado(&'static str),
    /// The text key is not this command's `cmd-<id>`.
    LabelDoesNotMatchId {
        /// The command's id.
        id: &'static str,
        /// The key it declared.
        label: &'static str,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdDuplicado(id) => write!(f, "the command `{id}` is registered twice"),
            Self::IdMalFormado(id) => write!(
                f,
                "command id `{id}` is outside the `kebab-case` shape \
                 (lower case, digits and hyphen, no dot and no space)"
            ),
            Self::LabelDoesNotMatchId { id, label } => write!(
                f,
                "the command `{id}` points at the key `{label}`, and not at \
                 `{KEY_PREFIX_OF}{id}`.\n\
                 \n\
                 That compiles because the key exists — only it is ANOTHER \
                 command's, and which-key would show the wrong label with no error \
                 none."
            ),
        }
    }
}

impl std::error::Error for Failure {}

/// Every command the system knows about.
///
/// `BTreeMap` and not `HashMap`: the iteration order has to be deterministic
/// so `M-x` always lists the same way and so the tests do not depend on hashing.
#[derive(Debug, Default)]
pub struct Registry {
    commands: BTreeMap<&'static str, Command>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Adds a command.
    ///
    /// # Errors
    ///
    /// A repeated id or one outside the format. Both are programming errors,
    /// not user errors — which is why they fail loudly instead of being ignored.
    pub fn register(&mut self, c: Command) -> Result<(), Failure> {
        if !valid_id(c.id) {
            return Err(Failure::IdMalFormado(c.id));
        }
        // THE KEY IS THE ID, with a prefix. Checking here closes the one hole
        // the compiler does not see: `key::CMD_WINDOW_UP` on a command called
        // `window-down` compiles, and the list comes out with the wrong label.
        if !c
            .label
            .strip_prefix(KEY_PREFIX_OF)
            .is_some_and(|rest| rest == c.id)
        {
            return Err(Failure::LabelDoesNotMatchId {
                id: c.id,
                label: c.label,
            });
        }
        if self.commands.contains_key(c.id) {
            return Err(Failure::IdDuplicado(c.id));
        }
        self.commands.insert(c.id, c);
        Ok(())
    }

    /// Look up by id.
    #[must_use]
    pub fn search(&self, id: &str) -> Option<&Command> {
        self.commands.get(id)
    }

    /// All of them, in stable id order.
    pub fn all_ones(&self) -> impl Iterator<Item = &Command> {
        self.commands.values()
    }

    /// How many commands there are.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.commands.len()
    }
}

/// `kebab-case`: lower case, digits and hyphen, with no hyphen at either end and no double one.
///
/// # Why the dot went away
///
/// The id used to be `dominio.action` and the text key `cmd-dominio-action`: the same
/// information in two spellings, with nothing tying one to the other. With the id in kebab, the
/// key is the id with a prefix — and `register` can CHECK that, which was
/// impossible while the translation between the two spellings existed.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Command = Command {
        id: "buffer-trocar",
        label: "cmd-buffer-trocar",
    };

    #[test]
    fn a_repeated_id_is_an_error() {
        // Two modules registering the same id is an integration bug. Accepting it
        // silently would let the second one win, and the shortcut would point at the
        // wrong command with no warning at all.
        let mut r = Registry::fresh();
        r.register(A).unwrap();
        assert_eq!(r.register(A), Err(Failure::IdDuplicado("buffer-trocar")));
    }

    #[test]
    fn the_id_is_kebab_and_the_dot_is_gone() {
        // THE DOT IS REJECTED, and not merely tolerated. As long as `buffer.trocar` and
        // `buffer-trocar` were both valid, the id would stop being
        // derivable from the text key — which is the property
        // `LabelDoesNotMatchId` depends on to exist.
        let mut r = Registry::fresh();
        for bad in [
            "buffer.trocar",
            "Buffer-Trocar",
            "buffer trocar",
            "buffer-",
            "-x",
            "buffer--trocar",
            "",
        ] {
            assert!(
                r.register(Command {
                    id: bad,
                    label: "x"
                })
                .is_err(),
                "`{bad}` should have been refused"
            );
        }
    }

    #[test]
    fn the_key_has_to_be_the_commands_own() {
        // THE DEFECT THIS PREVENTS: copying a command line, changing the id
        // and forgetting the key. Both constants exist, so the compiler
        // accepts it, and which-key shows the neighbouring command's label — which is
        // indistinguishable from a translation bug.
        let mut r = Registry::fresh();
        assert_eq!(
            r.register(Command {
                id: "window-down",
                label: "cmd-window-up",
            }),
            Err(Failure::LabelDoesNotMatchId {
                id: "window-down",
                label: "cmd-window-up",
            })
        );
        // And the key without the prefix does not pass either.
        assert!(
            r.register(Command {
                id: "window-down",
                label: "window-down",
            })
            .is_err()
        );
    }

    #[test]
    fn the_order_is_stable() {
        // `M-x` lists from here. Hash order would make the list change between
        // runs, and the user would lose their muscle memory.
        let mut r = Registry::fresh();
        for (id, label) in [("z-z", "cmd-z-z"), ("a-a", "cmd-a-a"), ("m-m", "cmd-m-m")] {
            r.register(Command { id, label }).unwrap();
        }
        let ids: Vec<_> = r.all_ones().map(|c| c.id).collect();
        assert_eq!(ids, ["a-a", "m-m", "z-z"]);
    }
}
