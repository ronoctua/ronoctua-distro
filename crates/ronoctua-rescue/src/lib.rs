//! The rescue console — what `Ctrl+Alt+Del` opens.
//!
//! # Why it exists
//!
//! The footer is this system's ONLY chrome (invariant 14). When it dies there is
//! no modeline, no minibuffer and no leader; when the compositor dies there is
//! nothing at all. The three binds that live outside the leader
//! (`crates/ronoctua-shell/src/actions.rs`) cover three cases, and all three ask
//! the COMPOSITOR to spawn — so all three are gone exactly when the compositor
//! is the thing that broke.
//!
//! This is the door that works when nothing works.
//!
//! # Two doors, one screen
//!
//! With niri alive, `Ctrl+Alt+Del` opens a floating terminal window — the same
//! mechanism `Ctrl+Alt+Esc` already uses for the process monitor, so the mouse
//! works. With niri dead, systemd opens the same binary on a VT of its own,
//! through `ctrl-alt-del.target`: no Wayland, no GPU, no font atlas, no
//! compositor. The screen is the same one; what changes is whether a pointer
//! exists.
//!
//! That is why this is a terminal application and not a Wayland client. A
//! rescue window that needs the compositor to be alive is a lock inside the
//! safe.
//!
//! # An action is DATA
//!
//! Every one-click fix in here is a row in [`all_of`] — an id, a text key, what
//! to run, and whether it needs the person to prove who they are. Nothing about
//! WHAT to run lives in the drawing code, and that is what makes the list
//! auditable from Nix: `rescue-actions-have-text` reads the ids out of this file
//! and demands the key in both languages, and `rescue-polkit-is-narrow` demands
//! that the units the [`Tier::Password`] rows name are exactly the ones the
//! polkit rule allows — no more.
//!
//! It is the same principle as `ronoctua-cmd`'s registry, and for the same
//! reason: a table can be checked, a `match` arm cannot.

#![forbid(unsafe_code)]

pub mod action;
pub mod edit;
pub mod tui;
pub mod view;

/// The text key prefix of every action.
///
/// The key and the id are the SAME thing written twice — see
/// [`Failure::LabelDoesNotMatchId`], which is what stops them from drifting.
pub const KEY_PREFIX_OF: &str = "rescue-";

/// Whether the person has to prove who they are before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Runs with no question asked.
    ///
    /// Two kinds end up here, and both for the same reason: they cannot be used
    /// against the owner. Reading (the log, the process list, a config file)
    /// changes nothing; and restarting a unit of the person's OWN session is
    /// something anyone sitting at an unlocked machine could do by killing the
    /// process anyway — `systemctl --user` does not even reach polkit.
    ///
    /// It matters that this tier is generous: when the keyboard is what broke,
    /// typing a password is exactly what one cannot do, and a diagnostic behind
    /// a password is a diagnostic that is not there.
    Free,
    /// Asks for the person's own password, once, and remembers it.
    ///
    /// Everything that touches the SYSTEM, gives a shell, or cannot be undone.
    /// The asking is polkit's (`auth_self_keep`), not ours — this crate never
    /// sees a password, and a rescue console that implemented authentication
    /// would be the worst possible place to get authentication wrong.
    Password,
}

/// What choosing a row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// `systemctl --user <verb> <units…>`.
    ///
    /// The session's own units. There is no display manager on this system
    /// (`nix/modules/niri.nix`): the compositor and the footer are USER units,
    /// so restarting them needs no privilege at all.
    User {
        /// `restart`, `stop`, `start`.
        verb: &'static str,
        /// The units, in the order systemd should get them.
        units: &'static [&'static str],
    },
    /// `systemctl <verb> <units…>` — the system's, and polkit decides.
    System {
        /// `restart` or `start`.
        verb: &'static str,
        /// The units. `rescue-polkit-is-narrow` compares this list against the
        /// rule, both ways.
        units: &'static [&'static str],
    },
    /// What logind owns, and what `Ctrl+Alt+Del` used to do by itself.
    Power(Power),
    /// What the app does in its own process.
    Own(Own),
}

/// Leaving the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// `systemctl reboot`.
    Reboot,
    /// `systemctl poweroff`.
    Off,
}

/// The rows the application answers for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Own {
    /// Ask the compositor to close the focused window (`CloseWindow`).
    CloseFocused,
    /// SIGTERM the process behind the window chosen in the list.
    ///
    /// Distinct from the one above because they fail differently: a window that
    /// ignores `CloseWindow` is a program that stopped reading its event loop,
    /// and the only thing left is the signal.
    KillChosen,
    /// Stop whatever is eating the most memory.
    KillBiggest,
    /// Hand the screen to a shell, and take it back when the shell exits.
    Shell,
    /// Open this machine's configuration for editing.
    EditHost,
    /// Rebuild the font cache.
    ///
    /// Its own row and not a shell command, because the failure it answers —
    /// every glyph on screen turning into a box — looks like a corrupted
    /// install, and whoever is looking at it should not have to know the name
    /// of the tool.
    RefreshFonts,
}

/// The seven entries of the sidebar, left to right of the screen's split.
///
/// The sidebar is DATA and not a drawing: every row of [`all_of`] declares where
/// it lives, so adding a row cannot leave it unreachable — which is the failure
/// a hand-written menu has, and it fails silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// What is standing and what is not. It runs nothing.
    State,
    /// The one-key fixes.
    Actions,
    /// The compositor's windows, to close or to kill.
    Windows,
    /// A shell, in full screen.
    Terminals,
    /// The configuration files — the one that is editable, and the generated ones.
    Config,
    /// The journal, by unit. It runs nothing.
    Log,
    /// Generations, rollback, and the machine's power.
    System,
}

impl Section {
    /// The sidebar, in order.
    #[must_use]
    pub const fn all_of() -> [Self; 7] {
        [
            Self::State,
            Self::Actions,
            Self::Windows,
            Self::Terminals,
            Self::Config,
            Self::Log,
            Self::System,
        ]
    }

    /// Its text key — `rescue-section-<name>`, and never the word itself.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::State => "rescue-section-state",
            Self::Actions => "rescue-section-actions",
            Self::Windows => "rescue-section-windows",
            Self::Terminals => "rescue-section-terminals",
            Self::Config => "rescue-section-config",
            Self::Log => "rescue-section-log",
            Self::System => "rescue-section-system",
        }
    }
}

/// A row of the rescue console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// A stable id, in `kebab-case`.
    pub id: &'static str,
    /// A **key** of text, never the sentence (invariant 8).
    ///
    /// Always `rescue-<id>`, and [`check_of`] refuses anything else. Written out
    /// rather than derived for the reason `ronoctua-cmd` gives: a generated
    /// constant is checked by the compiler, a `String` built at run time is not.
    pub label: &'static str,
    /// What it does.
    pub run: Run,
    /// Whether it asks first.
    pub tier: Tier,
    /// Where in the sidebar it appears.
    pub section: Section,
}

/// The rows of one section, in the table's order.
#[must_use]
pub fn of_section(section: Section) -> Vec<&'static Action> {
    all_of().iter().filter(|a| a.section == section).collect()
}

/// Why a table is not a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Two rows with the same id.
    IdRepeated(&'static str),
    /// An id outside `kebab-case`.
    IdMalformed(&'static str),
    /// The text key is not this row's `rescue-<id>`.
    LabelDoesNotMatchId {
        /// The row's id.
        id: &'static str,
        /// The key it carries.
        label: &'static str,
    },
    /// A privileged row that names no unit, or a free row that names a system one.
    TierDoesNotMatchRun(&'static str),
}

/// The rescue console's rows, in the order they appear.
///
/// # How each one earned its place
///
/// Not one of them is here because it sounded useful: each answers a failure
/// this repository has already met and written down. The order is the order one
/// reaches for them — the cheapest and most reversible first, the machine's
/// power last.
#[must_use]
pub const fn all_of() -> &'static [Action] {
    &[
        // THE FOOTER, and it is first because it is the likeliest.
        //
        // It is the only chrome there is: dead, the person has no modeline, no
        // minibuffer and no leader — and the desktop looks perfectly fine, which
        // is what makes it hard to name from the inside.
        Action {
            id: "restart-footer",
            label: "rescue-restart-footer",
            run: Run::User {
                verb: "restart",
                units: &["ronoctua-footer.service"],
            },
            tier: Tier::Free,
            section: Section::Actions,
        },
        // THE COMPOSITOR. It takes the windows with it, which is why it is not
        // first — but a frozen niri is a frozen screen, and nothing else fixes it.
        Action {
            id: "restart-niri",
            label: "rescue-restart-niri",
            run: Run::User {
                verb: "restart",
                units: &["niri.service"],
            },
            tier: Tier::Free,
            section: Section::Actions,
        },
        // THE WHOLE SESSION, when restarting the two above was not enough.
        // `bindsTo`/`partOf` in `nix/modules/niri.nix` make everything follow
        // this target, which is what "the graphical environment" means here —
        // there is no display manager to restart.
        Action {
            id: "restart-session",
            label: "rescue-restart-session",
            run: Run::User {
                verb: "restart",
                units: &["graphical-session.target"],
            },
            tier: Tier::Free,
            section: Section::Actions,
        },
        // SOUND AFTER A SUSPEND, which is the classic one that looks like broken
        // hardware and is three user units that came back in the wrong order.
        Action {
            id: "restart-audio",
            label: "rescue-restart-audio",
            run: Run::User {
                verb: "restart",
                units: &[
                    "pipewire.service",
                    "pipewire-pulse.service",
                    "wireplumber.service",
                ],
            },
            tier: Tier::Free,
            section: Section::Actions,
        },
        // THE KEYBOARD. `keyd` sits between the keyboard and everything else: if
        // it falls over, Super stops emitting F13 and the leader stops existing.
        //
        // Free tier, and not by our choice: `nix/modules/keyboard.nix` already
        // has a polkit rule allowing exactly this user to restart exactly this
        // unit, and `Ctrl+Shift+L` already fires it without asking anything. A
        // password here would be a password on the recovery path for a broken
        // keyboard.
        Action {
            id: "restore-keyboard",
            label: "rescue-restore-keyboard",
            run: Run::System {
                verb: "restart",
                units: &["keyd.service"],
            },
            tier: Tier::Free,
            section: Section::Actions,
        },
        // THE POLITE REQUEST to the window that will not go away.
        Action {
            id: "close-window",
            label: "rescue-close-window",
            run: Run::Own(Own::CloseFocused),
            tier: Tier::Free,
            section: Section::Actions,
        },
        // TOFU. The font cache goes stale and every glyph turns into a box —
        // the failure that looks like a corrupted install and is a cache.
        Action {
            id: "refresh-fonts",
            label: "rescue-refresh-fonts",
            run: Run::Own(Own::RefreshFonts),
            tier: Tier::Free,
            section: Section::Actions,
        },
        // ---- from here on, it asks ----

        // A SHELL, for everything that does not fit in a row above.
        Action {
            id: "open-terminal",
            label: "rescue-open-terminal",
            run: Run::Own(Own::Shell),
            tier: Tier::Password,
            section: Section::Terminals,
        },
        // THE SIGNAL, when the request was ignored.
        Action {
            id: "kill-window",
            label: "rescue-kill-window",
            run: Run::Own(Own::KillChosen),
            tier: Tier::Password,
            section: Section::Windows,
        },
        // THRASHING. `nix/modules/memory.nix` keeps the desktop alive under
        // pressure; this is the hand that ends the cause.
        Action {
            id: "kill-hog",
            label: "rescue-kill-hog",
            run: Run::Own(Own::KillBiggest),
            tier: Tier::Password,
            section: Section::Actions,
        },
        // THE ONE CONFIGURATION FILE THIS MACHINE OWNS.
        Action {
            id: "edit-host",
            label: "rescue-edit-host",
            run: Run::Own(Own::EditHost),
            tier: Tier::Password,
            section: Section::Config,
        },
        // AND APPLYING IT. Editing without this is editing a file that changes
        // nothing, which is worse than not editing.
        Action {
            id: "rebuild-switch",
            label: "rescue-rebuild-switch",
            run: Run::System {
                verb: "start",
                units: &["ronoctua-rescue-rebuild.service"],
            },
            tier: Tier::Password,
            section: Section::Config,
        },
        // THE WAY BACK. A rebuild that breaks the session leaves a machine that
        // boots into nothing, and the previous generation is right there — but
        // until now nothing in this repository could reach it without a shell.
        Action {
            id: "rollback-now",
            label: "rescue-rollback-now",
            run: Run::System {
                verb: "start",
                units: &["ronoctua-rescue-rollback@switch.service"],
            },
            tier: Tier::Password,
            section: Section::System,
        },
        // The same, at the next boot, for when switching NOW is the risk: the
        // generation that works may be the one that cannot be switched into
        // from a broken session.
        Action {
            id: "rollback-boot",
            label: "rescue-rollback-boot",
            run: Run::System {
                verb: "start",
                units: &["ronoctua-rescue-rollback@boot.service"],
            },
            tier: Tier::Password,
            section: Section::System,
        },
        // THE NETWORK, which is how one asks anyone else for help.
        Action {
            id: "restart-network",
            label: "rescue-restart-network",
            run: Run::System {
                verb: "restart",
                units: &["NetworkManager.service"],
            },
            tier: Tier::Password,
            section: Section::Actions,
        },
        // AND THE TWO THAT `Ctrl+Alt+Del` USED TO DO BY ITSELF.
        //
        // They are last, and they ask. Taking the key over means taking the
        // responsibility for what it did: whoever pressed it wanting a reboot
        // finds the reboot here, one row down, instead of losing the session
        // mid-sentence.
        Action {
            id: "reboot",
            label: "rescue-reboot",
            run: Run::Power(Power::Reboot),
            tier: Tier::Password,
            section: Section::System,
        },
        Action {
            id: "poweroff",
            label: "rescue-poweroff",
            run: Run::Power(Power::Off),
            tier: Tier::Password,
            section: Section::System,
        },
    ]
}

/// Every system unit a [`Tier::Password`] row names, deduplicated and sorted.
///
/// It is what `rescue-polkit-is-narrow` compares the rule against: the polkit
/// file may allow these and nothing else. A row added here without the rule is
/// an action that fails in front of the person; a rule kept after the row left
/// is privilege granted to nobody, which is worse — it is the kind of leftover
/// nobody audits.
#[must_use]
pub fn privileged_units() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = all_of()
        .iter()
        .filter(|a| a.tier == Tier::Password)
        .filter_map(|a| match a.run {
            Run::System { units, .. } => Some(units),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether the table holds together.
///
/// # Errors
///
/// The first thing that is wrong, and nothing after it — a list of complaints
/// about one broken row reads like several broken rows.
pub fn check_of(rows: &[Action]) -> Result<(), Failure> {
    let mut seen: Vec<&'static str> = Vec::with_capacity(rows.len());
    for a in rows {
        if !kebab(a.id) {
            return Err(Failure::IdMalformed(a.id));
        }
        if a.label.strip_prefix(KEY_PREFIX_OF) != Some(a.id) {
            return Err(Failure::LabelDoesNotMatchId {
                id: a.id,
                label: a.label,
            });
        }
        if seen.contains(&a.id) {
            return Err(Failure::IdRepeated(a.id));
        }
        // A privileged row that names no unit and does nothing of its own would
        // ask for a password and then have nothing to do with it.
        let coherent = match (a.tier, a.run) {
            (Tier::Password, Run::User { .. }) => false,
            (_, Run::System { units, .. }) => !units.is_empty(),
            _ => true,
        };
        if !coherent {
            return Err(Failure::TierDoesNotMatchRun(a.id));
        }
        seen.push(a.id);
    }
    Ok(())
}

/// `kebab-case`: lowercase, digits and hyphens, never at the ends, never doubled.
fn kebab(id: &str) -> bool {
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

    #[test]
    fn the_table_holds_together() {
        check_of(all_of()).expect("the rescue's table");
    }

    #[test]
    fn a_label_that_belongs_to_another_row_is_refused() {
        // THE DEFECT THIS CATCHES, and it compiles perfectly: the key exists —
        // it is just somebody else's, and the screen would draw the wrong
        // sentence with nothing at all going wrong.
        let wrong = [Action {
            id: "reboot",
            label: "rescue-poweroff",
            run: Run::Power(Power::Reboot),
            tier: Tier::Password,
            section: Section::System,
        }];
        assert_eq!(
            check_of(&wrong),
            Err(Failure::LabelDoesNotMatchId {
                id: "reboot",
                label: "rescue-poweroff",
            })
        );
    }

    #[test]
    fn a_privileged_row_has_to_have_something_to_ask_for() {
        let wrong = [Action {
            id: "restart-footer",
            label: "rescue-restart-footer",
            run: Run::User {
                verb: "restart",
                units: &["ronoctua-footer.service"],
            },
            tier: Tier::Password,
            section: Section::Actions,
        }];
        assert_eq!(
            check_of(&wrong),
            Err(Failure::TierDoesNotMatchRun("restart-footer"))
        );
    }

    #[test]
    fn the_free_tier_reaches_no_system_unit_but_the_keyboard() {
        // THE LINE THIS DRAWS, and why it is worth a test: everything in the
        // free tier is either the person's own session or reading. The single
        // exception is `keyd`, and it is not an exception we granted — the
        // keyboard module's polkit rule already allows exactly it, and
        // `Ctrl+Shift+L` already fires it without asking.
        for a in all_of().iter().filter(|a| a.tier == Tier::Free) {
            if let Run::System { units, .. } = a.run {
                assert_eq!(
                    units,
                    ["keyd.service"],
                    "`{}` reaches a system unit without asking anything",
                    a.id
                );
            }
            assert!(
                !matches!(a.run, Run::Power(_)),
                "`{}` shuts the machine down without asking for anything",
                a.id
            );
        }
    }

    #[test]
    fn every_privileged_unit_is_named_once() {
        let units = privileged_units();
        let mut sorted = units.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            units, sorted,
            "the list has to come sorted and with no repeats"
        );
        assert!(
            units.contains(&"ronoctua-rescue-rollback@switch.service"),
            "the rollback left the list the polkit rule mirrors: {units:?}"
        );
    }
}
