//! The keymap as the shell's crates contribute it, and the one each process
//! installs (shell-data-design §1.6).
//!
//! # Why the keymap is assembled from contributions
//!
//! The default keymap was one function of the footer's leader, holding every
//! command, sequence and group of every view: the module that walks the
//! sequences named every feature, and a line of the installer stick's table
//! was a line of the footer. Now each crate says its own keys in a
//! [`Contribution`] beside the view that answers them — its commands, the
//! sequences and groups that reach them, the ones only `LEADER :` reaches,
//! and what the view's own keys and rows run — the footer says the window's,
//! the workspace's and its own (`ronoctua_shell_footer::leader::BASE`), and the binary
//! assembles them (its `keymap`), the one place that names them all.
//! [`assemble`] refuses what `default_of` refused — two commands under one id,
//! a binding to a command nobody registers, a binding and its command that
//! disagree about the argument, every collision of every set — and what a
//! table per crate invites: a crate binding, or naming as reached by name
//! only, another crate's command.
//!
//! # Why an installed registry
//!
//! A sentence that teaches a key reads it from the tree the leader walks,
//! never from text (`keys-are-never-text`), and a door used to build the
//! whole keymap to ask it. A view's crate cannot build it any more — the
//! keymap names every crate — so the binary [`install`]s its builder at
//! start, beside the theme and the date's shape, and a door asks [`taught`]:
//! the keymap is built once, on the first question, in the process that
//! asks — the footer's too, which walks the same trees on every key
//! ([`installed`]).
//!
//! # Why the trees and the sets live here
//!
//! [`Set`], [`Trees`] and the folding are what [`taught`] answers from, and
//! they name nothing but the keymap's own crate: the footer's leader walks
//! them, and every door teaches from them.

use std::collections::BTreeSet;

use anyhow::Result;
use ronoctua_cmd::{Command, Registry};
use ronoctua_keymap::{Binding, GroupNode, Tree, parse_sequence};

/// One sequence of a contribution: the keys after the leader (`"s z z"`),
/// the command they run, and the fixed argument it carries, when the command
/// takes one (`workspace-go` and its number).
pub type Row = (&'static str, &'static str, Option<&'static str>);

/// A generated sequence: [`Row`] with keys built at assembly (`TAB 3`).
pub type GeneratedRow = (String, &'static str, Option<&'static str>);

/// What a contribution builds when the keymap is assembled, rather than
/// writes: the footer's nine workspace digits, the window menu's levels, the
/// compositor's deeds and the brain modules' actions.
#[derive(Debug, Default)]
pub struct Generated {
    /// Commands registered beside the written ones.
    pub commands: Vec<Command>,
    /// Sequences bound like the written ones: a duplicate is a collision.
    pub bindings: Vec<GeneratedRow>,
    /// Sequences that ADOPT a key: one identical to a sequence already bound
    /// — same keys, command and argument — is not bound twice, and one that
    /// gives the keys another command is a collision. The window menu's,
    /// whose cells take the keys `w` already binds to the same deed.
    pub adopting: Vec<GeneratedRow>,
    /// Groups: their keys and the key of their label.
    pub groups: Vec<(String, &'static str)>,
}

/// What one crate adds to the leader's keymap.
///
/// Every field is the crate's own: its commands, the sequences that run
/// them (a contribution binds only what it registers, which [`assemble`]
/// holds it to), the groups it names, the ones `LEADER :` alone reaches and
/// why, what it generates, and what its view's own keys and rows run. A group
/// may hold another crate's sequences: `s` is the footer's and `s z z` the
/// installer stick's.
#[derive(Debug, Clone, Copy)]
pub struct Contribution {
    /// The crate's name, for what [`assemble`] refuses.
    pub owner: &'static str,
    /// Its commands, each labelled by its own `cmd-<id>` key.
    pub commands: &'static [Command],
    /// The sequences that run them.
    pub bindings: &'static [Row],
    /// The groups it names: their keys and the key of their label.
    pub groups: &'static [(&'static str, &'static str)],
    /// The commands it registers that only `LEADER :` reaches — a list of
    /// names, each with its reason beside it, and never a prefix: `brain-`
    /// as a prefix would excuse the next brain command somebody forgets to
    /// bind.
    pub by_name: &'static [&'static str],
    /// What it generates at assembly.
    pub generated: fn() -> Generated,
    /// What its view's own keys and rows run, which no sequence of the tree
    /// does: `d` in a list removes, and is not `LEADER d`.
    pub reached: fn() -> Vec<&'static str>,
}

/// Nothing generated.
fn nothing() -> Generated {
    Generated::default()
}

/// Nothing reached outside the tree.
const fn nowhere() -> Vec<&'static str> {
    Vec::new()
}

impl Contribution {
    /// The empty contribution, for the fields a crate leaves out
    /// (`..Contribution::NONE`).
    pub const NONE: Self = Self {
        owner: "",
        commands: &[],
        bindings: &[],
        groups: &[],
        by_name: &[],
        generated: nothing,
        reached: nowhere,
    };
}

/// The trees of the three sets and the registry of every command, assembled
/// from every crate's contribution.
///
/// # Errors
///
/// A command registered twice or outside the `kebab-case` shape, a binding
/// pointing at a command nobody registers or that another crate registers, a
/// command named as reached by name only that is not the crate's own, a
/// binding and its command that disagree about the argument, or a collision
/// in any of the three sets — all of them programming errors, which is why
/// the footer refuses to start on one and `check-keymap` (what `nix flake
/// check` runs) names it.
pub fn assemble(all: &[&Contribution]) -> Result<(Trees, Registry)> {
    let generated: Vec<Generated> = all.iter().map(|c| (c.generated)()).collect();
    let mut registry = Registry::fresh();
    // WHOSE EACH COMMAND IS: the contribution that registers it.
    let mut owner: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for (n, (c, g)) in all.iter().zip(&generated).enumerate() {
        for command in c.commands.iter().chain(&g.commands) {
            registry
                .register(*command)
                .map_err(|e| anyhow::anyhow!("{e} (in the keys of `{}`)", c.owner))?;
            owner.insert(command.id, n);
        }
    }

    // A CRATE BINDS, AND NAMES AS REACHED BY NAME ONLY, WHAT IT REGISTERS:
    // the keys of a view live beside the view that answers them, and a
    // sequence one crate writes for another's command would be a key nobody
    // finds where the command is.
    let whose = |id: &str| owner.get(id).map(|&o| all[o].owner);
    for (n, (c, g)) in all.iter().zip(&generated).enumerate() {
        let rows = c
            .bindings
            .iter()
            .map(|(seq, id, _)| (*seq, *id))
            .chain(g.bindings.iter().map(|(seq, id, _)| (seq.as_str(), *id)))
            .chain(g.adopting.iter().map(|(seq, id, _)| (seq.as_str(), *id)));
        for (seq, id) in rows {
            // NOBODY'S is refused below, as a binding to nothing.
            if let Some(&o) = owner.get(id)
                && o != n
            {
                anyhow::bail!(
                    "`{}` binds `{seq}` to `{id}`, which is `{}`'s: a crate binds only \
                     the commands it registers",
                    c.owner,
                    all[o].owner
                );
            }
        }
        for id in c.by_name {
            anyhow::ensure!(
                owner.get(id) == Some(&n),
                "`{}` names `{id}` as reached by name only, and it is {}",
                c.owner,
                whose(id).map_or_else(|| "nobody's".to_owned(), |o| format!("`{o}`'s"))
            );
        }
    }

    // THE WRITTEN SEQUENCES FIRST, every crate's, then the generated ones —
    // and what ADOPTS a key is bound only where nothing identical already is.
    let written: Vec<GeneratedRow> = all
        .iter()
        .flat_map(|c| c.bindings.iter().map(|(s, c, a)| ((*s).to_owned(), *c, *a)))
        .chain(generated.iter().flat_map(|g| g.bindings.iter().cloned()))
        .collect();
    let adopted: Vec<GeneratedRow> = generated
        .iter()
        .flat_map(|g| g.adopting.iter())
        .filter(|g| !written.contains(g))
        .cloned()
        .collect();
    let mut bindings = Vec::with_capacity(written.len() + adopted.len());
    for (seq, command, argument) in written.into_iter().chain(adopted) {
        let cmd = registry.search(command).ok_or_else(|| {
            anyhow::anyhow!(
                "the binding `{seq}` points at the command `{command}`, which does not exist"
            )
        })?;
        // A COMMAND WITH AN ARGUMENT ONLY ACCEPTS A BINDING WITH AN ARGUMENT, and the
        // converse too: `workspace-go` with no number does not know where to go, and
        // `buffer-close` with a number receives something it silently ignores.
        anyhow::ensure!(
            crate::actions::with_argument(command).is_some() == argument.is_some(),
            "the binding `{seq}` and the command `{command}` disagree about the argument"
        );
        bindings.push(Binding {
            sequence: parse_sequence(&seq)?,
            command,
            label: cmd.label,
            argument,
        });
    }

    let groups: std::result::Result<Vec<_>, _> = all
        .iter()
        .flat_map(|c| c.groups.iter().map(|(seq, label)| (*seq, *label)))
        .chain(
            generated
                .iter()
                .flat_map(|g| g.groups.iter().map(|(seq, label)| (seq.as_str(), *label))),
        )
        .map(|(seq, label)| parse_sequence(seq).map(|sequence| GroupNode { sequence, label }))
        .collect();

    // THE THREE SETS, each built and each verified: the contributions are
    // the full tree as written, and the other two are derived from it
    // (`collapse`) — see `Set` for why there are three.
    let trees = Trees::build(&bindings, &groups?).map_err(|collisions| {
        let list = collisions
            .iter()
            .map(|(set, c)| format!("  - in the {set:?} set: {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::anyhow!("the default keymap has a collision:\n{list}")
    })?;

    Ok((trees, registry))
}

/// Every command SOME key in this system reaches.
///
/// # Why it is not the keymap alone
///
/// The keymap resolves SEQUENCES, and sequences are not the only source of
/// keys here. The **chords** live in the compositor's config (the footer never
/// sees them — §7.4); the **emergency** binds and the keyboard's top row live
/// outside the leader on purpose; and every VIEW that takes the keyboard has a
/// table of its own, because `d` in a list removes and is not `LEADER d` —
/// each contribution's [`Contribution::reached`]. Asking "does this have a
/// shortcut?" of the keymap alone answers wrongly for most of them, which is
/// what the orphan check would then report.
#[must_use]
pub fn reachable(tree: &Tree, all: &[&Contribution]) -> BTreeSet<&'static str> {
    let mut out: BTreeSet<&'static str> = tree.reachable_commands().into_iter().collect();
    out.extend(crate::actions::full_chords().into_iter().map(|a| a.command));
    out.extend(
        crate::actions::emergency()
            .into_iter()
            .map(|(_, _, id)| id)
            // The alternative leader is not a command: it fires the sequence.
            .filter(|id| *id != crate::actions::LEADER_ALTERNATE),
    );
    // THE KEYBOARD'S TOP ROW, which the compositor binds directly — the same
    // arrangement as the emergency keys just above.
    out.extend(crate::actions::media().into_iter().map(|(_, _, id)| id));
    for c in all {
        out.extend((c.reached)());
    }
    out
}

/// The commands reachable by NAME and by name only, every contribution's
/// [`Contribution::by_name`].
///
/// Two places read it: `check-keymap`, which is what `nix flake check` runs,
/// and the test beside it, which is what `cargo test` runs — the same list,
/// so the day they disagreed cannot come.
#[must_use]
pub fn by_name_only(all: &[&Contribution]) -> Vec<&'static str> {
    all.iter().flat_map(|c| c.by_name.iter().copied()).collect()
}

/// WHICH OF THE THREE TREES a sequence walks, chosen by pressing the LEADER
/// again while its list is open.
///
/// # Why there are three
///
/// The owner's request (2026-09-28): the key that used to open an app's
/// SUBMENU opens the app itself, in the footer — `LEADER s n` is the network
/// where it was `LEADER s n n`. The same key one set further opens the app's
/// WINDOW, and one further still the whole tree of before is there, every
/// submenu as it was. The two doors of an app were only ever a convention of
/// NAMES — the doubled letter is the footer, `w` the window — and that
/// convention is what decides which groups fold: see [`collapse`].
///
/// # Why the leader again, and no modifier (2026-09-30)
///
/// The sets were a TAP of `Alt` or `Ctrl` after the leader. The owner took
/// both away from them: a tap of `Ctrl` now cycles the bars' modes
/// (`ronoctua_shell_footer::hidden::next_mode`), and `Alt` chooses nothing. The leader itself
/// is the one key every hand already has under it when the list is open, and
/// pressing it again walks the sets in order — default, windows, the whole
/// tree, and back to the default ([`Set::next`]) — so `LEADER LEADER s n` is
/// the network's window and `LEADER LEADER LEADER s n a` the aeroplane.
/// Holding a modifier while typing the letter was never an answer: `M-` is the
/// compositor's family (`AGENTS.md`), and in the nested loop the leader's own
/// `Ctrl` and `Alt` are HELD by construction when the sequence starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Set {
    /// `LEADER` alone: a group with two doors opens its footer door.
    #[default]
    Footer,
    /// `LEADER LEADER`: the same key opens the app's window.
    Window,
    /// `LEADER LEADER LEADER`: the whole tree, every submenu as it was.
    Full,
}

/// The letter a group's SECOND door hangs off — `w`, for window, the letter
/// every view with a window uses (`. w`, `s n w`, `s u w`). The first door is
/// the group's own last key doubled. [`folds`] reads both.
pub const WINDOW_DOOR_KEY: &str = "w";

impl Set {
    /// How many times the leader is pressed AGAIN to reach this set from a
    /// fresh opening: none for the default.
    #[must_use]
    pub const fn presses(self) -> usize {
        match self {
            Self::Footer => 0,
            Self::Window => 1,
            Self::Full => 2,
        }
    }

    /// The key a sequence in this set is shown with after the leader's, when
    /// there is one — the leader again, once per [`Self::presses`], under the
    /// label `leader` it goes by (`SUPER`, or `CTRL ALT SPC` in the nested
    /// loop). The default set is the leader alone.
    #[must_use]
    pub fn key_with(self, leader: &str) -> Option<String> {
        (self.presses() > 0).then(|| vec![leader; self.presses()].join(" "))
    }

    /// The set the leader pressed again goes to: default, windows, the whole
    /// tree, and round to the default.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Footer => Self::Window,
            Self::Window => Self::Full,
            Self::Full => Self::Footer,
        }
    }

    /// The command that switches to this set — what the POINTER runs, since
    /// a click cannot press the leader (invariant 19). Its id is also the key
    /// of the short word the hint line draws beside the key.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Footer => "leader-set-default",
            Self::Window => "leader-set-window",
            Self::Full => "leader-set-full",
        }
    }

    /// The three, in the order teaching looks: the default first.
    pub const ALL: [Self; 3] = [Self::Footer, Self::Window, Self::Full];
}

/// The three trees, built once and each verified.
///
/// Three [`Tree`]s and not one tree with a switch inside: each goes through
/// [`Tree::build_it`], so a collision the folding would create — a folded
/// group landing on a key that is a prefix of something — is a BUILD error
/// in the set it happens in, exactly as it is in the full one.
#[derive(Debug, Default)]
pub struct Trees {
    /// [`Set::Footer`]'s — the one a sequence starts in.
    pub footer: Tree,
    /// [`Set::Window`]'s.
    pub window: Tree,
    /// [`Set::Full`]'s: every contribution's sequences and groups as
    /// written, unchanged.
    pub full: Tree,
}

impl Trees {
    /// The tree of one set.
    #[must_use]
    pub const fn of(&self, set: Set) -> &Tree {
        match set {
            Set::Footer => &self.footer,
            Set::Window => &self.window,
            Set::Full => &self.full,
        }
    }

    /// The three trees of one list of bindings and groups.
    ///
    /// # Errors
    ///
    /// EVERY collision of every set, each with the set it is in — the rule
    /// [`Tree::build_it`] keeps, of fixing a keymap in one build rather than
    /// in three.
    pub fn build(
        bindings: &[Binding],
        groups: &[GroupNode],
    ) -> std::result::Result<Self, Vec<(Set, ronoctua_keymap::Collision)>> {
        let mut failed = Vec::new();
        let mut one = |set: Set| {
            let (b, g) = collapse(bindings, groups, set);
            Tree::build_it(&b, &g)
                .map_err(|all| failed.extend(all.into_iter().map(|c| (set, c))))
                .ok()
        };
        let (footer, window, full) = (one(Set::Footer), one(Set::Window), one(Set::Full));
        match (footer, window, full) {
            (Some(footer), Some(window), Some(full)) if failed.is_empty() => Ok(Self {
                footer,
                window,
                full,
            }),
            _ => Err(failed),
        }
    }
}

/// A group that FOLDS into one key: its sequence, and its two doors.
#[derive(Debug, Clone)]
pub struct Fold {
    /// The group's own sequence — the key it becomes (`s n`).
    pub group: Vec<ronoctua_keymap::Key>,
    /// The footer's door: the group's last key doubled (`s n n`).
    pub footer: Binding,
    /// The window's door: [`WINDOW_DOOR_KEY`] under the group (`s n w`).
    pub window: Binding,
}

/// Every group that folds in [`Set::Footer`] and [`Set::Window`].
///
/// # The rule, and why it is read and not listed
///
/// A group G folds when BOTH `G <G's last key>` and `G w` are bindings, to two
/// DIFFERENT commands: that is the two-door convention, and it holds, with no
/// exception, for every app that has a footer and a window. A list of the
/// groups kept by hand beside the tables would be one more thing to forget
/// when an app gains a window; the test `the_collapsed_groups_are_these` pins
/// what the rule answers today, so a new group that folds is a decision
/// somebody makes on purpose.
///
/// `w` itself does not fold, and the rule says why without an exception: its
/// doubled key IS its `w` (`w w`), one binding and not two doors. A group
/// with only one door (`RET`, `TAB`, `s q`), or with none (`m`, `s`, `d`),
/// does not fold either.
///
/// # And nothing under `w` folds
///
/// `w` IS the window's group, and the groups under it are the window menu's
/// levels, generated from the menu with the menu's own letters
/// (`window_menu::leader_bindings`). There `w` is one more letter of a level
/// and never a second door: `w v v` selects the window and `w v w` its whole
/// workspace, two deeds of one level that the rule above, read alone, took
/// for a footer and a window — the pinned test caught it.
#[must_use]
pub fn folds(bindings: &[Binding], groups: &[GroupNode]) -> Vec<Fold> {
    let Ok(window_key) = ronoctua_keymap::Key::parse(WINDOW_DOOR_KEY) else {
        return Vec::new();
    };
    let bound = |seq: &[ronoctua_keymap::Key]| bindings.iter().find(|b| b.sequence == seq);
    let mut out: Vec<Fold> = Vec::new();
    for g in groups {
        let Some(last) = g.sequence.last().copied() else {
            continue;
        };
        if g.sequence.first() == Some(&window_key) || out.iter().any(|f| f.group == g.sequence) {
            continue;
        }
        let doubled: Vec<_> = g.sequence.iter().copied().chain([last]).collect();
        let windowed: Vec<_> = g.sequence.iter().copied().chain([window_key]).collect();
        if doubled == windowed {
            continue;
        }
        let (Some(footer), Some(window)) = (bound(&doubled), bound(&windowed)) else {
            continue;
        };
        if (footer.command, footer.argument) == (window.command, window.argument) {
            continue;
        }
        out.push(Fold {
            group: g.sequence.clone(),
            footer: footer.clone(),
            window: window.clone(),
        });
    }
    out
}

/// The bindings and groups of one set, derived from the full ones.
///
/// [`Set::Full`] is the input unchanged. In the other two, every group that
/// [`folds`] becomes ONE binding — to its footer's door in [`Set::Footer`],
/// to its window's in [`Set::Window`] — and everything under it, bindings and
/// groups, leaves that set: the other keys of a folded group (`s n a`, the
/// aeroplane) are still two more presses of the leader away. A fold inside a folded group
/// is the outer one's to take away.
#[must_use]
pub fn collapse(
    bindings: &[Binding],
    groups: &[GroupNode],
    set: Set,
) -> (Vec<Binding>, Vec<GroupNode>) {
    if set == Set::Full {
        return (bindings.to_vec(), groups.to_vec());
    }
    let all = folds(bindings, groups);
    // THE OUTERMOST ONLY: a fold under another fold is already gone.
    let outer: Vec<&Fold> = all
        .iter()
        .filter(|f| {
            !all.iter()
                .any(|o| o.group.len() < f.group.len() && f.group.starts_with(&o.group))
        })
        .collect();
    let under = |seq: &[ronoctua_keymap::Key]| outer.iter().any(|f| seq.starts_with(&f.group));
    let mut out: Vec<Binding> = bindings
        .iter()
        .filter(|b| !under(&b.sequence))
        .cloned()
        .collect();
    for f in &outer {
        let door = if set == Set::Footer {
            &f.footer
        } else {
            &f.window
        };
        out.push(Binding {
            sequence: f.group.clone(),
            ..door.clone()
        });
    }
    let kept = groups
        .iter()
        .filter(|g| !under(&g.sequence))
        .cloned()
        .collect();
    (out, kept)
}

/// The keys that run `command` — with `argument` — the shortest way from the
/// DEFAULT set, written as a sentence teaches them after [`label`].
///
/// The footer's own sequence when it has one (`t`, the queue); else the
/// window set's, after the leader again (`SUPER .`, the manager's window, so
/// `LEADER SUPER .` on screen); else the full tree's, after the leader twice
/// more (`SUPER SUPER s b y`, answering a pairing). A key taught any other way
/// is a key that does something else when typed — `t t` in the default set is
/// the queue and then a key that leads nowhere.
///
/// `None` for a command no sequence reaches.
#[must_use]
pub fn taught_keys(trees: &Trees, command: &str, argument: Option<&str>) -> Option<String> {
    taught_keys_with(trees, command, argument, &label())
}

/// [`taught_keys`] under a given leader label — the one the tests pin.
#[must_use]
pub fn taught_keys_with(
    trees: &Trees,
    command: &str,
    argument: Option<&str>,
    leader: &str,
) -> Option<String> {
    Set::ALL.into_iter().find_map(|set| {
        let keys = ronoctua_keymap::notation(&trees.of(set).sequence_of(command, argument)?);
        Some(match set.key_with(leader) {
            Some(k) => format!("{k} {keys}"),
            None => keys,
        })
    })
}

/// How the leader appears on screen.
///
/// # Why it comes from the ENVIRONMENT
///
/// It used to be a fixed `"SPC"`, and that was wrong in both modes: in the nested loop the
/// key is `Ctrl+Alt+Space` (the `Mod` belongs to the host's compositor) and in
/// production it is `Super` ON ITS OWN, tapped. The screen said `SPC` in both — a
/// label that corresponds to no key does not help, it gets in the way.
///
/// Whoever defines the bind defines the label: `nix/dev.nix` in the nested loop,
/// `nix/modules/scroll.nix` in production. A single truth.
///
/// Without the variable, it falls back to `SPC` — Doom's notation, which is where the
/// concept comes from, and which serves whoever runs the binary by hand.
#[must_use]
pub fn label() -> String {
    std::env::var("RONOCTUA_LEADER").unwrap_or_else(|_| "SPC".to_owned())
}

/// How the CHORD's key appears on screen.
///
/// # Why it is another variable
///
/// In production it is the same key as the sequence's — tapping Super opens the list,
/// holding Super is the modifier — and it is that coincidence `keyd` buys.
/// In the nested loop it is **not**: the sequence is `Ctrl+Alt+Space` and the chord is `Alt`
/// alone. Deriving one from the other would make the screen say one holds `CTRL ALT SPC`,
/// and holding Space along with it is exactly what fires the sequence — the
/// opposite of what the chord needs.
///
/// Without the variable, it falls back to the leader's label: outside the two known modes,
/// repeating what one knows is better than inventing a second key.
#[must_use]
pub fn chord_label() -> String {
    std::env::var("RONOCTUA_LEADER_CHORD").unwrap_or_else(|_| label())
}

/// What builds this process's keymap: the binary's assembly of every crate's
/// contribution (the binary's `keymap::build`).
pub type Builder = fn() -> Result<(Trees, Registry)>;

/// The builder [`install`] was given.
static BUILDER: std::sync::OnceLock<Builder> = std::sync::OnceLock::new();

/// The keymap the builder built, on the first question — or why it did not.
static BUILT: std::sync::OnceLock<std::result::Result<(Trees, Registry), String>> =
    std::sync::OnceLock::new();

/// Installs this process's keymap: what [`installed`], [`read`] and
/// [`taught`] answer from. The binary calls it once, at start; a second call
/// changes nothing.
pub fn install(builder: Builder) {
    let _ = BUILDER.set(builder);
}

/// This process's keymap: built by the installed builder on the first
/// question, and the same one ever after — the footer's, which walks it on
/// every key, and the one every door teaches from ([`taught`]).
///
/// # Errors
///
/// No keymap was installed in this process, or it does not build (a
/// programming error, see [`assemble`]); the failure is logged once, when it
/// happens.
pub fn installed() -> Result<&'static (Trees, Registry)> {
    let builder = BUILDER
        .get()
        .ok_or_else(|| anyhow::anyhow!("no keymap was installed in this process"))?;
    let built = BUILT.get_or_init(|| {
        builder().map_err(|e| {
            tracing::error!(error = %e, "the keymap did not build");
            format!("{e:#}")
        })
    });
    built
        .as_ref()
        .map_err(|e| anyhow::anyhow!("the keymap did not build: {e}"))
}

/// Asks this process's keymap ([`installed`]) — or, in a test, the one the
/// test thread was given ([`with_keymap`], [`with_stub`]).
///
/// # Errors
///
/// As [`installed`].
pub fn read<R>(f: impl FnOnce(&Trees, &Registry) -> R) -> Result<R> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(stub) = STUB.with(|s| s.borrow().clone()) {
        return Ok(f(&stub.0, &stub.1));
    }
    let (trees, registry) = installed()?;
    Ok(f(trees, registry))
}

/// The keys that run `command` — with `argument` — after the leader, the
/// shortest way from the default set ([`taught_keys`] over the installed
/// keymap): `t` for the queue, `SUPER .` for the file manager's window.
/// `None` when no sequence reaches it, or no keymap is installed.
#[must_use]
pub fn keys(command: &str, argument: Option<&str>) -> Option<String> {
    read(|trees, _| taught_keys(trees, command, argument))
        .ok()
        .flatten()
}

/// What a sentence teaches for `command`: the leader's [`label`] and its
/// [`keys`] (`SUPER t`) — an argument of the sentence, never its text
/// (`keys-are-never-text`). `None` when no sequence reaches it.
#[must_use]
pub fn taught(command: &str, argument: Option<&str>) -> Option<String> {
    keys(command, argument).map(|k| format!("{} {k}", label()))
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// The keymap this test thread reads instead of the installed one.
    static STUB: std::cell::RefCell<Option<std::rc::Rc<(Trees, Registry)>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `f` with `keymap` as what [`read`], [`keys`] and [`taught`] answer
/// from on this thread — the binary's tests, over the real assembly. Nothing
/// installs a keymap in a test binary, so a test that reads one without this
/// reads none.
#[cfg(any(test, feature = "test-support"))]
pub fn with_keymap<R>(keymap: (Trees, Registry), f: impl FnOnce() -> R) -> R {
    struct Restore(Option<std::rc::Rc<(Trees, Registry)>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            STUB.with(|s| *s.borrow_mut() = before);
        }
    }
    let _restore = Restore(STUB.with(|s| s.replace(Some(std::rc::Rc::new(keymap)))));
    f()
}

/// Runs `f` over a keymap of these sequences alone (`("t t",
/// "transfers-open")`) — every prefix a group, so the sets fold as the real
/// ones do, and an empty registry. For a crate's test that teaches a key
/// without the binary that assembles them all.
#[cfg(any(test, feature = "test-support"))]
pub fn with_stub<R>(rows: &[(&'static str, &'static str)], f: impl FnOnce() -> R) -> R {
    let bindings: Vec<Binding> = rows
        .iter()
        .map(|(seq, command)| Binding {
            sequence: parse_sequence(seq).expect("a stub's sequence parses"),
            command,
            label: command,
            argument: None,
        })
        .collect();
    let mut groups: Vec<GroupNode> = Vec::new();
    for b in &bindings {
        for n in 1..b.sequence.len() {
            let sequence = b.sequence[..n].to_vec();
            if !groups.iter().any(|g| g.sequence == sequence) {
                groups.push(GroupNode {
                    sequence,
                    label: "keymap.no-label",
                });
            }
        }
    }
    let trees = Trees::build(&bindings, &groups).expect("a stub keymap collides");
    with_keymap((trees, Registry::fresh()), f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key;

    /// A binding for the rule's own tests — a CALL and not a tuple, so the
    /// made-up ids here are not read as keys of the real table by the checks
    /// that sweep this file for `("seq", "id"` rows.
    fn bind(seq: &str, command: &'static str) -> Binding {
        Binding {
            sequence: parse_sequence(seq).unwrap(),
            command,
            label: "x",
            argument: None,
        }
    }

    fn group(seq: &str) -> GroupNode {
        GroupNode {
            sequence: parse_sequence(seq).unwrap(),
            label: "x",
        }
    }

    /// FAILS IF: the rule folds a group it should not — one door, one
    /// binding answering for both, the same command twice — or keeps one it
    /// should fold, or a fold inside a fold survives its outer one.
    #[test]
    fn a_group_folds_when_it_has_two_different_doors() {
        let bindings = [
            bind("a a", "a-footer"),
            bind("a w", "a-window"),
            bind("a x", "a-other"),
            // `w`'s doubled key IS its `w`: one binding, not two doors.
            bind("w w", "w-both"),
            // AND UNDER `w` THE LETTER `w` IS NO DOOR: a level of the window
            // menu with two deeds that happen to sit on `v` and `w`.
            bind("w v v", "v-one"),
            bind("w v w", "v-other"),
            // ONE DOOR.
            bind("b b", "b-footer"),
            bind("b x", "b-other"),
            // TWO KEYS, ONE COMMAND.
            bind("c c", "c-same"),
            bind("c w", "c-same"),
            // A FOLD INSIDE A FOLD.
            bind("d d", "d-footer"),
            bind("d w", "d-window"),
            bind("d e e", "e-footer"),
            bind("d e w", "e-window"),
        ];
        let groups = [
            group("a"),
            group("w"),
            group("w v"),
            group("b"),
            group("c"),
            group("d"),
            group("d e"),
        ];
        let folded: Vec<String> = folds(&bindings, &groups)
            .iter()
            .map(|f| ronoctua_keymap::notation(&f.group))
            .collect();
        assert_eq!(folded, ["a", "d", "d e"]);

        let tree = |set| {
            let (b, g) = collapse(&bindings, &groups, set);
            Tree::build_it(&b, &g).expect("a folded tree collides")
        };
        let runs = |t: &Tree, keys: &str| t.resolve(&parse_sequence(keys).unwrap());
        let (footer, window, full) = (tree(Set::Footer), tree(Set::Window), tree(Set::Full));
        use ronoctua_keymap::Resolution::{Command, Nothing, Prefix};
        assert_eq!(runs(&footer, "a"), Command("a-footer", None));
        assert_eq!(runs(&window, "a"), Command("a-window", None));
        // EVERYTHING UNDER A FOLD LEAVES THE SET, and the fold inside it with it.
        assert_eq!(runs(&footer, "a x"), Nothing);
        assert_eq!(runs(&footer, "d"), Command("d-footer", None));
        assert_eq!(runs(&window, "d"), Command("d-window", None));
        assert_eq!(runs(&footer, "d e"), Nothing);
        // WHAT DOES NOT FOLD IS THE SAME IN ALL THREE.
        for t in [&footer, &window, &full] {
            assert_eq!(runs(t, "w w"), Command("w-both", None));
            assert_eq!(runs(t, "w v w"), Command("v-other", None));
            assert_eq!(runs(t, "b b"), Command("b-footer", None));
            assert_eq!(runs(t, "c w"), Command("c-same", None));
            assert!(matches!(runs(t, "b"), Prefix(_)));
        }
        // AND THE FULL SET IS THE TABLE AS WRITTEN.
        assert!(matches!(runs(&full, "a"), Prefix(_)));
        assert_eq!(runs(&full, "a x"), Command("a-other", None));
        assert_eq!(runs(&full, "d e e"), Command("e-footer", None));
    }

    /// FAILS IF: the leader pressed again stops walking the three sets in
    /// order — default, windows, the whole tree, and round to the default —
    /// or a set's key stops being the leader once per press.
    #[test]
    fn the_sets_walk_round() {
        assert_eq!(Set::Footer.next(), Set::Window);
        assert_eq!(Set::Window.next(), Set::Full);
        assert_eq!(Set::Full.next(), Set::Footer);
        assert_eq!(Set::Footer.key_with("SUPER"), None);
        assert_eq!(Set::Window.key_with("SUPER").as_deref(), Some("SUPER"));
        assert_eq!(Set::Full.key_with("SUPER").as_deref(), Some("SUPER SUPER"));
    }

    const FILES: Contribution = Contribution {
        owner: "files",
        commands: &[
            Command {
                id: "files-open",
                label: key::CMD_FILES_OPEN,
            },
            Command {
                id: "files-window",
                label: key::CMD_FILES_WINDOW,
            },
        ],
        bindings: &[(". .", "files-open", None), (". w", "files-window", None)],
        groups: &[(".", key::GROUP_FILE)],
        ..Contribution::NONE
    };

    /// The same command again, under another crate's name.
    const AGAIN: Contribution = Contribution {
        owner: "again",
        commands: &[Command {
            id: "files-open",
            label: key::CMD_FILES_OPEN,
        }],
        ..Contribution::NONE
    };

    /// A sequence to a command nobody registers.
    const STRAY: Contribution = Contribution {
        owner: "stray",
        bindings: &[("z", "files-nowhere", None)],
        ..Contribution::NONE
    };

    /// A command that takes a number, bound without one.
    const BARE: Contribution = Contribution {
        owner: "bare",
        commands: &[Command {
            id: "workspace-go",
            label: key::CMD_WORKSPACE_GO,
        }],
        bindings: &[("TAB 3", "workspace-go", None)],
        ..Contribution::NONE
    };

    /// A sequence of one crate for another's command.
    const BORROWED: Contribution = Contribution {
        owner: "borrowed",
        bindings: &[("z", "files-open", None)],
        ..Contribution::NONE
    };

    /// Another crate's command, named as reached by name only.
    const NAMED: Contribution = Contribution {
        owner: "named",
        by_name: &["files-open"],
        ..Contribution::NONE
    };

    /// Another crate's sequence on the manager's own keys.
    const CLASH: Contribution = Contribution {
        owner: "clash",
        commands: &[Command {
            id: "buffer-next",
            label: key::CMD_BUFFER_NEXT,
        }],
        bindings: &[(". .", "buffer-next", None)],
        ..Contribution::NONE
    };

    fn adopting() -> Generated {
        Generated {
            // THE MANAGER'S OWN KEY, ADOPTED: bound once.
            adopting: vec![(". .".to_owned(), "files-open", None)],
            ..Generated::default()
        }
    }

    fn adopting_another() -> Generated {
        Generated {
            // THE MANAGER'S KEY FOR ANOTHER COMMAND: a collision.
            adopting: vec![(". w".to_owned(), "files-open", None)],
            ..Generated::default()
        }
    }

    /// FAILS IF: the assembly takes what the single table refused — one
    /// command registered by two crates, a sequence to a command nobody
    /// registers, a command that takes an argument bound without one, two
    /// crates' sequences on one key — or what a table of its own invites: a
    /// crate's sequence, or its "by name only", for another crate's command;
    /// or refuses what it should take: one crate's keys, and a generated
    /// sequence that adopts a key already bound to the same command.
    #[test]
    fn the_assembly_refuses_what_the_table_refused() {
        let error = |all: &[&Contribution]| match assemble(all) {
            Ok(_) => panic!(
                "{:?} assembled",
                all.iter().map(|c| c.owner).collect::<Vec<_>>()
            ),
            Err(e) => format!("{e:#}"),
        };
        let twice = error(&[&FILES, &AGAIN]);
        assert!(
            twice.contains("files-open") && twice.contains("`again`"),
            "{twice}"
        );
        let stray = error(&[&FILES, &STRAY]);
        assert!(
            stray.contains("`files-nowhere`, which does not exist"),
            "{stray}"
        );
        let bare = error(&[&BARE]);
        assert!(bare.contains("disagree about the argument"), "{bare}");
        let clash = error(&[&FILES, &CLASH]);
        assert!(clash.contains("collision"), "{clash}");
        let borrowed = error(&[&FILES, &BORROWED]);
        assert!(borrowed.contains("which is `files`'s"), "{borrowed}");
        let named = error(&[&FILES, &NAMED]);
        assert!(named.contains("`named` names `files-open`"), "{named}");
        let nobody = error(&[&NAMED]);
        assert!(nobody.contains("nobody's"), "{nobody}");

        let (trees, registry) = assemble(&[&FILES]).expect("one crate's keys");
        assert_eq!(registry.amount(), 2);
        assert_eq!(
            taught_keys_with(&trees, "files-window", None, "SUPER").as_deref(),
            Some("SUPER .")
        );
        let adopted = Contribution {
            generated: adopting,
            ..FILES
        };
        let (trees, _) = assemble(&[&adopted]).expect("an adopted key is bound once");
        assert_eq!(trees.full.amount(), 2);
        let other = Contribution {
            generated: adopting_another,
            ..FILES
        };
        assert!(error(&[&other]).contains("collision"));
    }

    /// FAILS IF: a stub stops teaching from the default set (the queue is
    /// `t` where `t t` and `t w` fold), the whole tree's keys stop coming
    /// after the leader twice more, a stub outlives its closure or leaks into
    /// the one it was nested in, or a test binary — where nothing installs a
    /// keymap — answers a key nobody gave it.
    #[test]
    fn a_stub_teaches_like_the_keymap_and_only_inside() {
        let leader = label();
        assert_eq!(keys("transfers-open", None), None);
        let rows = [
            ("t t", "transfers-open"),
            ("t w", "transfers-window"),
            ("s b b", "bluetooth-open"),
            ("s b w", "bluetooth-window"),
            ("s b y", "bluetooth-accept"),
        ];
        with_stub(&rows, || {
            assert_eq!(keys("transfers-open", None).as_deref(), Some("t"));
            assert_eq!(taught("transfers-open", None), Some(format!("{leader} t")));
            assert_eq!(keys("transfers-window", None), Some(format!("{leader} t")));
            assert_eq!(
                keys("bluetooth-accept", None),
                Some(format!("{leader} {leader} s b y"))
            );
            assert_eq!(keys("files-open", None), None);
            with_stub(&[("o", "files-open")], || {
                assert_eq!(keys("files-open", None).as_deref(), Some("o"));
                assert_eq!(keys("transfers-open", None), None);
            });
            assert_eq!(keys("transfers-open", None).as_deref(), Some("t"));
            assert!(read(|_, registry| registry.amount() == 0).expect("a stub reads"));
        });
        assert_eq!(keys("transfers-open", None), None);
        assert!(read(|_, _| ()).is_err());
    }
}
