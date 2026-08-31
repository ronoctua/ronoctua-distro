//! Key tree, merge, resolution and which-key — section 7.4.
//!
//! # The model
//!
//! A **sequence** of keys leads to a command. `SPC f f` is three keys;
//! `SPC` and `SPC f` are prefixes of it. As long as what has been typed is only
//! a prefix, the surface shows which-key: the possible continuations.
//!
//! # What this crate does NOT do
//!
//! It does not talk to Wayland, does not know what a surface is and does not draw. It
//! is a pure function of (bindings, typed sequence) to a [`Resolution`] —
//! which makes the keymap's state machine testable without a compositor, and that is
//! where most shortcut bugs live.
//!
//! # A collision is a BUILD error, not a runtime one
//!
//! [`Tree::build_it`] returns `Err` with **all** the collisions at once.
//! Two bindings on the same sequence, or a command hanging off a prefix that
//! another binding uses, are ambiguity — and ambiguity in a keymap shows up
//! as "sometimes it works", which is the worst possible symptom to diagnose.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

/// A key's modifiers.
///
/// `logo` is the Super/Meta key. In nested mode it belongs to the host's
/// compositor (see `nix/dev.nix`), which is why the development leader uses
/// `Alt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Mods {
    /// Control.
    pub ctrl: bool,
    /// Alt / Meta.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Super / Logo.
    pub logo: bool,
}

/// A key's symbol, without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sym {
    /// A printable character.
    Char(char),
    /// Space. Written `SPC` — it is the leader.
    Space,
    /// `RET`.
    Enter,
    /// `TAB`.
    Tab,
    /// `ESC`. It always cancels (section 7.4).
    Esc,
    /// `DEL`.
    Backspace,
    /// Left arrow. Written `<left>`, as in Emacs.
    ///
    /// # Why the arrows came to exist
    ///
    /// They did NOT exist, and that was a decision: the project's notation is Emacs's,
    /// where `C-n` and `C-p` are the canonical movement. What changed is that the
    /// compositor's focus shortcuts live in the keymap now, and there the arrow is
    /// the gesture the hand already makes — `hjkl` still exists, in parallel.
    ///
    /// The `<>` notation is Emacs's for a key with no character. It is
    /// longer than a letter and that is why which-key reserves columns for
    /// the key; writing `←` would be shorter and would depend on the font having the
    /// glyph in every situation, including in the installer's TTY.
    Left,
    /// Right arrow, `<right>`.
    Right,
    /// Up arrow, `<up>`.
    Up,
    /// Down arrow, `<down>`.
    Down,
}

/// A key: modifiers plus symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// Modifiers held.
    pub mods: Mods,
    /// The symbol.
    pub sym: Sym,
}

impl Key {
    /// A key with no modifiers.
    #[must_use]
    pub const fn simple(sym: Sym) -> Self {
        Self {
            mods: Mods {
                ctrl: false,
                alt: false,
                shift: false,
                logo: false,
            },
            sym,
        }
    }

    /// Parses the Emacs/Doom notation: `SPC`, `f`, `C-x`, `M-x`, `s-RET`.
    ///
    /// # Errors
    ///
    /// An unknown modifier prefix, or a symbol that is neither a known
    /// name nor a single character.
    pub fn parse(s: &str) -> Result<Self, Failure> {
        let mut mods = Mods::default();
        let mut rest = s;

        // Modifiers are `X-` prefixes. The loop has to stop when what
        // is left IS a one-character symbol followed by `-`: `C--` is Control
        // with the hyphen itself, not Control plus an empty modifier.
        while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
            match rest.as_bytes()[0] {
                b'C' => mods.ctrl = true,
                b'M' => mods.alt = true,
                b'S' => mods.shift = true,
                b's' => mods.logo = true,
                _ => return Err(Failure::UnknownModifier(s.to_owned())),
            }
            rest = &rest[2..];
        }

        let sym = match rest {
            "SPC" => Sym::Space,
            "RET" => Sym::Enter,
            "TAB" => Sym::Tab,
            "ESC" => Sym::Esc,
            "DEL" => Sym::Backspace,
            "<left>" => Sym::Left,
            "<right>" => Sym::Right,
            "<up>" => Sym::Up,
            "<down>" => Sym::Down,
            otherwise => {
                let mut chars = otherwise.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Sym::Char(c),
                    _ => return Err(Failure::UnknownSymbol(s.to_owned())),
                }
            }
        };
        Ok(Self { mods, sym })
    }

    /// How the key appears in which-key and in the modeline.
    ///
    /// **It is not UI text**: it is the key's own notation, the same in every
    /// language, like Doom's `SPC`. Invariant 8 holds for sentences, not
    /// for a key's name.
    #[must_use]
    pub fn notation(&self) -> String {
        let mut s = String::with_capacity(8);
        if self.mods.ctrl {
            s.push_str("C-");
        }
        if self.mods.alt {
            s.push_str("M-");
        }
        if self.mods.shift {
            s.push_str("S-");
        }
        if self.mods.logo {
            s.push_str("s-");
        }
        match self.sym {
            Sym::Char(c) => s.push(c),
            Sym::Space => s.push_str("SPC"),
            Sym::Enter => s.push_str("RET"),
            Sym::Tab => s.push_str("TAB"),
            Sym::Esc => s.push_str("ESC"),
            Sym::Backspace => s.push_str("DEL"),
            Sym::Left => s.push_str("<left>"),
            Sym::Right => s.push_str("<right>"),
            Sym::Up => s.push_str("<up>"),
            Sym::Down => s.push_str("<down>"),
        }
        s
    }
}

/// Error while building the keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// A modifier prefix that does not exist.
    UnknownModifier(String),
    /// A key name that does not exist.
    UnknownSymbol(String),
    /// An empty sequence — there is nothing to bind.
    EmptySequence,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownModifier(s) => write!(f, "an unknown modifier in `{s}`"),
            Self::UnknownSymbol(s) => write!(f, "an unknown key in `{s}`"),
            Self::EmptySequence => write!(f, "an empty key sequence"),
        }
    }
}

impl std::error::Error for Failure {}

/// Parses a whole sequence: `"SPC f f"`.
///
/// # Errors
///
/// An empty sequence, or any malformed key.
pub fn parse_sequence(s: &str) -> Result<Vec<Key>, Failure> {
    let keys: Result<Vec<_>, _> = s.split_whitespace().map(Key::parse).collect();
    let keys = keys?;
    if keys.is_empty() {
        return Err(Failure::EmptySequence);
    }
    Ok(keys)
}

/// A binding: a sequence leads to a command.
#[derive(Debug, Clone)]
pub struct Binding {
    /// The complete sequence, starting from the leader.
    pub sequence: Vec<Key>,
    /// The id of a `ronoctua-cmd` command.
    pub command: &'static str,
    /// The **key** of the label's text in which-key (invariant 8).
    pub label: &'static str,
    /// A fixed argument for the command, when it requires one.
    ///
    /// # Why the binding carries this
    ///
    /// `LEADER TAB 3` goes to workspace 3, and `LEADER TAB 7` to 7: it is the
    /// SAME command with different arguments. Without the argument here, the
    /// alternative would be nine commands (`workspace-ir-1`…`-9`) that exist only
    /// to carry a number — nine names in the locator saying the same
    /// thing, and nine text keys to translate.
    ///
    /// The chord already did this: `LEADER+3` carries the `3` in the KDL itself. Here the
    /// sequence gains the same ability.
    pub argument: Option<&'static str>,
}

/// A prefix group, with a name of its own in which-key: `SPC f` = "file".
#[derive(Debug, Clone)]
pub struct GroupNode {
    /// The prefix's sequence.
    pub sequence: Vec<Key>,
    /// The **key** of the group name's text.
    pub label: &'static str,
}

/// A collision between bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Collision {
    /// Two bindings on the same sequence.
    Duplicate {
        /// The sequence, in notation.
        sequence: String,
        /// The two commands contending.
        commands: (&'static str, &'static str),
    },
    /// A command hanging off a sequence that is also a prefix of another.
    ///
    /// `SPC f` as a command and `SPC f f` as another: typing `SPC f` becomes
    /// ambiguous — wait for another key or execute? There is no good answer, so
    /// it is an error.
    PrefixWithCommand {
        /// The prefix in contention.
        sequence: String,
        /// The command hanging off it.
        command: &'static str,
        /// A longer binding that passes through it.
        extends: &'static str,
    },
}

impl std::fmt::Display for Collision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate { sequence, commands } => write!(
                f,
                "`{sequence}` esta ligada duas vezes: `{}` e `{}`",
                commands.0, commands.1
            ),
            Self::PrefixWithCommand {
                sequence,
                command,
                extends,
            } => write!(
                f,
                "`{sequence}` is a command (`{command}`) AND a prefix of `{extends}`; \
                 typing the sequence would be ambiguous"
            ),
        }
    }
}

/// What a typed sequence means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Nothing matches. The right thing is to cancel and say so.
    Nothing,
    /// Still a prefix: these are the continuations, for which-key.
    Prefix(Vec<Entry>),
    /// A command matched, with its fixed argument when there is one.
    Command(&'static str, Option<&'static str>),
}

/// One which-key row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The key to press, already in notation.
    pub key: String,
    /// The SAME key, as data.
    ///
    /// It exists because the list became clickable: clicking a row is the same
    /// as pressing its key, and without this the shell would have to RE-PARSE
    /// the notation this module has just produced — a round trip through text
    /// that would lose information on the way (`SPC` is ` `, `RET` is `\n`).
    pub sym: Key,
    /// The **key** of the label's text.
    ///
    /// Owned, and not `&'static str`, because of the FOCUSED BUFFER: it declares
    /// its bindings at runtime and the key of each one is built as `cmd-<id>`
    /// (section 7.9). A borrowed type here would make the grafting in
    /// `Footer::view` impossible and the buffer's keymap would stay invisible —
    /// declared, resolved by its own process, and absent from which-key.
    pub label: String,
    /// The command it runs, when it runs one.
    ///
    /// `None` on a group: it leads to another level, and there is nothing to name. It is
    /// what the list SHOWS today — the command's id, as in Doom — with the
    /// label going to the message area when the pointer passes over it.
    ///
    /// Owned for the same reason as `label`.
    pub command: Option<String>,
    /// Whether it leads to another level instead of executing.
    pub is_group: bool,
}

/// A node of the tree.
#[derive(Debug, Default)]
struct Node {
    children: BTreeMap<Key, Node>,
    command: Option<(&'static str, &'static str, Option<&'static str>)>,
    group: Option<&'static str>,
}

/// The key tree, built and verified.
#[derive(Debug, Default)]
pub struct Tree {
    root: Node,
}

impl Tree {
    /// Builds the tree from the bindings and the group names.
    ///
    /// The bindings arrive already MERGED by the caller (default + user): the merge
    /// is a superposition of lists, and doing it here would hide from the caller
    /// which layer won.
    ///
    /// # Errors
    ///
    /// It returns **every** collision found, not the first. Fixing a
    /// keymap one collision at a time, with a whole build between them, is expensive.
    pub fn build_it(bindings: &[Binding], groups: &[GroupNode]) -> Result<Self, Vec<Collision>> {
        let mut tree = Self::default();
        let mut collisions = Vec::new();

        for l in bindings {
            let mut no = &mut tree.root;
            for t in &l.sequence {
                no = no.children.entry(*t).or_default();
            }
            if let Some((old, _, _)) = no.command {
                collisions.push(Collision::Duplicate {
                    sequence: notation(&l.sequence),
                    commands: (old, l.command),
                });
            } else {
                no.command = Some((l.command, l.label, l.argument));
            }
        }

        for g in groups {
            let mut no = &mut tree.root;
            for t in &g.sequence {
                no = no.children.entry(*t).or_default();
            }
            no.group = Some(g.label);
        }

        // Second pass: a command hanging off a node that also has children.
        // It can only be known after inserting everything — the longer binding may
        // have come before the shorter one.
        check_prefixes(&tree.root, &mut Vec::new(), &mut collisions);

        if collisions.is_empty() {
            Ok(tree)
        } else {
            Err(collisions)
        }
    }

    /// What the typed sequence means right now.
    #[must_use]
    pub fn resolve(&self, seq: &[Key]) -> Resolution {
        let mut no = &self.root;
        for t in seq {
            match no.children.get(t) {
                Some(n) => no = n,
                None => return Resolution::Nothing,
            }
        }
        if let Some((id, _, arg)) = no.command {
            return Resolution::Command(id, arg);
        }
        if no.children.is_empty() {
            return Resolution::Nothing;
        }
        // The order is determined by the `BTreeMap`: which-key always comes out the same,
        // which is what allows muscle memory.
        Resolution::Prefix(
            no.children
                .iter()
                .map(|(t, child)| Entry {
                    key: t.notation(),
                    sym: *t,
                    label: child
                        .group
                        .or(child.command.map(|(_, r, _)| r))
                        .unwrap_or("keymap.no-label")
                        .to_owned(),
                    command: child.command.map(|(id, _, _)| id.to_owned()),
                    is_group: child.command.is_none(),
                })
                .collect(),
        )
    }

    /// Every command reachable by some sequence, in stable order.
    ///
    /// It exists for the check that prevents this file's most expensive defect:
    /// a binding that appears in which-key pointing at a command the
    /// shell does not handle. It "executes", falls into a log `warn`, and on screen
    /// nothing happens — indistinguishable from an unregistered key.
    #[must_use]
    pub fn reachable_commands(&self) -> Vec<&'static str> {
        fn gathers(no: &Node, output: &mut Vec<&'static str>) {
            if let Some((id, _, _)) = no.command {
                output.push(id);
            }
            for child in no.children.values() {
                gathers(child, output);
            }
        }
        let mut v = Vec::new();
        gathers(&self.root, &mut v);
        v.sort_unstable();
        v.dedup();
        v
    }

    /// How many bindings the tree has. Used by the checks.
    #[must_use]
    pub fn amount(&self) -> usize {
        fn counts_up(no: &Node) -> usize {
            usize::from(no.command.is_some()) + no.children.values().map(counts_up).sum::<usize>()
        }
        counts_up(&self.root)
    }
}

fn check_prefixes(no: &Node, path: &mut Vec<Key>, output: &mut Vec<Collision>) {
    if let Some((cmd, _, _)) = no.command
        && let Some(extends) = first_command_below(no)
    {
        output.push(Collision::PrefixWithCommand {
            sequence: notation(path),
            command: cmd,
            extends,
        });
    }
    for (t, child) in &no.children {
        path.push(*t);
        check_prefixes(child, path, output);
        path.pop();
    }
}

fn first_command_below(no: &Node) -> Option<&'static str> {
    for child in no.children.values() {
        if let Some((id, _, _)) = child.command {
            return Some(id);
        }
        if let Some(id) = first_command_below(child) {
            return Some(id);
        }
    }
    None
}

/// The notation of a whole sequence, space-separated.
#[must_use]
pub fn notation(seq: &[Key]) -> String {
    seq.iter().map(Key::notation).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lig(seq: &str, command: &'static str) -> Binding {
        Binding {
            argument: None,
            sequence: parse_sequence(seq).unwrap(),
            command,
            label: "x",
        }
    }

    #[test]
    fn dooms_notation_reads_and_rewrites_the_same() {
        for s in ["SPC", "f", "C-x", "M-x", "C-M-S-s-a", "RET", "ESC", "TAB"] {
            assert_eq!(Key::parse(s).unwrap().notation(), s, "it failed at {s}");
        }
    }

    #[test]
    fn control_hyphen_is_control_with_a_hyphen() {
        // `C--` is Control plus the hyphen itself. A naive loop of "while
        // the second byte is `-`" would consume the hyphen as a modifier and
        // be left with an empty symbol.
        let t = Key::parse("C--").unwrap();
        assert!(t.mods.ctrl);
        assert_eq!(t.sym, Sym::Char('-'));
    }

    #[test]
    fn an_empty_sequence_is_an_error() {
        assert_eq!(parse_sequence("   "), Err(Failure::EmptySequence));
    }

    #[test]
    fn a_duplicate_binding_is_a_collision() {
        let e = Tree::build_it(&[lig("SPC f f", "a.a"), lig("SPC f f", "b.b")], &[]).unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(matches!(e[0], Collision::Duplicate { .. }));
    }

    #[test]
    fn a_command_on_a_prefix_is_a_collision() {
        // `SPC f` executing AND being a prefix of `SPC f f`: typing `SPC f`
        // would have no right answer. This is the core of milestone 5's check.
        let e = Tree::build_it(&[lig("SPC f", "a.a"), lig("SPC f f", "b.b")], &[]).unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(matches!(e[0], Collision::PrefixWithCommand { .. }));
    }

    #[test]
    fn the_insertion_order_does_not_change_the_diagnosis() {
        // The long binding may come before the short one. If the check were done
        // during insertion, one of the two cases would go unnoticed.
        let short_before =
            Tree::build_it(&[lig("SPC f", "a.a"), lig("SPC f f", "b.b")], &[]).unwrap_err();
        let long_before =
            Tree::build_it(&[lig("SPC f f", "b.b"), lig("SPC f", "a.a")], &[]).unwrap_err();
        assert_eq!(short_before.len(), long_before.len());
    }

    #[test]
    fn every_collision_at_once() {
        // Fixing one at a time would cost a whole build per collision.
        let e = Tree::build_it(
            &[
                lig("SPC a", "a.a"),
                lig("SPC a", "a.b"),
                lig("SPC b", "b.a"),
                lig("SPC b", "b.b"),
            ],
            &[],
        )
        .unwrap_err();
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn a_prefix_resolves_to_which_key() {
        let tree = Tree::build_it(
            &[lig("SPC f f", "file.open"), lig("SPC f s", "file.save")],
            &[GroupNode {
                sequence: parse_sequence("SPC f").unwrap(),
                label: "group.file",
            }],
        )
        .unwrap();

        let spc = parse_sequence("SPC").unwrap();
        match tree.resolve(&spc) {
            Resolution::Prefix(e) => {
                assert_eq!(e.len(), 1);
                assert_eq!(e[0].key, "f");
                assert_eq!(e[0].label, "group.file");
                assert!(e[0].is_group);
            }
            otherwise => panic!("expected a prefix, got {otherwise:?}"),
        }

        let spc_f = parse_sequence("SPC f").unwrap();
        match tree.resolve(&spc_f) {
            Resolution::Prefix(e) => {
                let keys: Vec<_> = e.iter().map(|x| x.key.as_str()).collect();
                assert_eq!(keys, ["f", "s"]);
                assert!(!e[0].is_group);
            }
            otherwise => panic!("expected a prefix, got {otherwise:?}"),
        }

        assert_eq!(
            tree.resolve(&parse_sequence("SPC f f").unwrap()),
            Resolution::Command("file.open", None)
        );
        assert_eq!(
            tree.resolve(&parse_sequence("SPC z").unwrap()),
            Resolution::Nothing
        );
    }

    #[test]
    fn the_which_key_order_is_stable() {
        // Muscle memory depends on this. Hash order would change the list between
        // runs with the same keymap.
        let tree = Tree::build_it(
            &[
                lig("SPC z", "z.z"),
                lig("SPC a", "a.a"),
                lig("SPC m", "m.m"),
            ],
            &[],
        )
        .unwrap();
        let Resolution::Prefix(e) = tree.resolve(&parse_sequence("SPC").unwrap()) else {
            panic!("expected a prefix")
        };
        let keys: Vec<_> = e.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["a", "m", "z"]);
    }

    #[test]
    fn different_modifiers_are_different_keys() {
        // `C-x` and `x` must not collide; if they did, half the Emacs
        // keymaps would be impossible.
        let tree = Tree::build_it(&[lig("SPC x", "a.a"), lig("SPC C-x", "b.b")], &[]).unwrap();
        assert_eq!(tree.amount(), 2);
    }
}
