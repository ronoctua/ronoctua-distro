//! The screen's state, and every key it answers — with no terminal in sight.
//!
//! Split out from the drawing for the reason the installer's `State` is
//! (`crates/ronoctua-installer/src/tui.rs`): what a key does is the part worth
//! testing, and testing it through a terminal means not testing it. Everything
//! here is a pure function of what it is handed.

use std::path::{Path, PathBuf};

use crate::{Action, Own, Run, Section, of_section};

/// A configuration file the console can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paper {
    /// Where it is on the installed machine.
    pub path: &'static str,
    /// Which file of the flake generates it, when it is generated.
    ///
    /// `None` means the machine owns it and it can be edited. `Some(_)` means it
    /// is a symlink into the store: read-only by construction, and editing it
    /// would be imperative (invariant 6) and would vanish at the next rebuild.
    /// The console says which file it was born from instead of pretending.
    pub origin: Option<&'static str>,
}

/// The files, and what may be done to each.
///
/// The generated ones are here ON PURPOSE. The person looking for "the niri
/// config" is looking for the file whose name they know; finding it, and being
/// told in the same breath where its truth lives, is the difference between an
/// answer and a dead end.
#[must_use]
pub const fn papers() -> &'static [Paper] {
    &[
        Paper {
            path: "/etc/nixos/host.nix",
            origin: None,
        },
        Paper {
            path: "/etc/nixos/flake.nix",
            origin: None,
        },
        Paper {
            path: "/etc/niri/config.kdl",
            origin: Some("nix/modules/niri.nix"),
        },
        Paper {
            path: "/etc/ronoctua/memory-profiles.conf",
            origin: Some("nix/modules/memory.nix"),
        },
        Paper {
            path: "/etc/ronoctua/fallback-chain.json",
            origin: Some("nix/modules/fonts.nix"),
        },
        Paper {
            path: "/etc/xdg/ghostty/config",
            origin: Some("nix/modules/terminal.nix"),
        },
    ]
}

/// The units whose journal the console shows.
///
/// The first is the whole machine; the other three are the pieces whose silence
/// is what brought the person here.
#[must_use]
pub const fn logs() -> &'static [(&'static str, bool)] {
    &[
        ("", false),
        ("niri.service", true),
        ("ronoctua-footer.service", true),
        ("keyd.service", false),
    ]
}

/// A window, as the console needs it — id, what to call it, and whose process it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    /// The compositor's id.
    pub id: u64,
    /// Title, `app_id`, or the id itself. Never empty.
    pub name: String,
    /// The process behind it, when the compositor knows it.
    pub pid: Option<i32>,
}

/// What the caller must do after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing happened.
    Still,
    /// The screen changed; draw it again.
    Draw,
    /// Run this row.
    Run(&'static Action),
    /// Leave the alternate screen and hand the terminal over.
    Hand(Hand),
    /// Close the console.
    Leave,
}

/// What takes the screen over, and gives it back when it exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hand {
    /// A login shell.
    Shell,
    /// A file, opened for editing.
    Edit(PathBuf),
    /// A file, opened to be read and not written.
    Read(PathBuf),
    /// The journal of a unit — empty means the whole machine's.
    Journal(String),
    /// The process monitor.
    Processes,
}

/// The console's whole state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// Which sidebar entry is open.
    pub section: Section,
    /// Which row of the content is under the cursor.
    pub row: usize,
    /// Whether the cursor is on the sidebar rather than on the content.
    pub on_sidebar: bool,
    /// What the last row that ran had to say.
    pub said: Option<String>,
    /// A row that has been armed and needs the same key again.
    pub pending: Option<&'static str>,
    /// The windows, as of the last time the compositor answered.
    pub panes: Vec<Pane>,
}

impl Default for View {
    fn default() -> Self {
        Self::fresh()
    }
}

impl View {
    /// A console that has just opened.
    ///
    /// It opens on `State` and not on `Actions`, and that is a decision: whoever
    /// gets here does not yet know what is wrong, and a list of fixes offered
    /// before the diagnosis invites fixing the wrong thing.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            section: Section::State,
            row: 0,
            on_sidebar: true,
            said: None,
            pending: None,
            panes: Vec::new(),
        }
    }

    /// How many rows the open section has.
    #[must_use]
    pub fn rows(&self) -> usize {
        match self.section {
            Section::State | Section::Terminals => of_section(self.section).len().max(1),
            Section::Actions | Section::System => of_section(self.section).len(),
            Section::Windows => self.panes.len(),
            Section::Config => papers().len(),
            Section::Log => logs().len(),
        }
    }

    /// The row under the cursor, when the section has actions.
    #[must_use]
    pub fn action(&self) -> Option<&'static Action> {
        of_section(self.section).get(self.row).copied()
    }

    /// Whether a row cannot be taken back, and therefore arms before it fires.
    ///
    /// Two presses, and the second one is the same key: it is the rule the
    /// memory panel already uses. What makes a row irreversible is not the
    /// privilege — it is that the screen the person is reading disappears.
    #[must_use]
    pub fn arms(a: &Action) -> bool {
        matches!(a.run, Run::Power(_))
            || matches!(a.run, Run::Own(Own::KillChosen | Own::KillBiggest))
            || matches!(a.run, Run::System { units, .. } if units.iter().any(|u| u.starts_with("ronoctua-rescue-rollback")))
    }

    /// Applies a key, and says what the caller has to do about it.
    pub fn key(&mut self, k: Key) -> Step {
        // ANY key that is not the confirmation disarms. Doing otherwise would
        // leave a machine armed to reboot while somebody scrolls a log.
        let armed = self.pending.take();

        match k {
            Key::Leave => Step::Leave,
            Key::Sideways => {
                self.on_sidebar = !self.on_sidebar;
                self.row = 0;
                Step::Draw
            }
            Key::Down => {
                self.move_by(1);
                Step::Draw
            }
            Key::Up => {
                self.move_by(-1);
                Step::Draw
            }
            Key::Pick(section) => {
                self.section = section;
                self.row = 0;
                self.on_sidebar = false;
                Step::Draw
            }
            Key::Enter => self.enter(armed),
        }
    }

    /// Moves the cursor, on whichever side of the split it is.
    fn move_by(&mut self, by: isize) {
        if self.on_sidebar {
            let all = Section::all_of();
            let at = all.iter().position(|s| *s == self.section).unwrap_or(0);
            let next = (at as isize + by).rem_euclid(all.len() as isize) as usize;
            self.section = all[next];
            self.row = 0;
            return;
        }
        let total = self.rows();
        if total == 0 {
            self.row = 0;
            return;
        }
        self.row = (self.row as isize + by).rem_euclid(total as isize) as usize;
    }

    /// What `Enter` does, which depends on which side and which section.
    fn enter(&mut self, armed: Option<&'static str>) -> Step {
        if self.on_sidebar {
            self.on_sidebar = false;
            self.row = 0;
            return Step::Draw;
        }
        match self.section {
            Section::Config => {
                let Some(p) = papers().get(self.row) else {
                    return Step::Still;
                };
                let path = PathBuf::from(p.path);
                Step::Hand(if p.origin.is_some() {
                    Hand::Read(path)
                } else {
                    Hand::Edit(path)
                })
            }
            Section::Log => {
                let Some((unit, _)) = logs().get(self.row) else {
                    return Step::Still;
                };
                Step::Hand(Hand::Journal((*unit).to_owned()))
            }
            Section::State => Step::Hand(Hand::Processes),
            _ => {
                let Some(a) = self.action() else {
                    return Step::Still;
                };
                if matches!(a.run, Run::Own(Own::Shell)) {
                    return Step::Hand(Hand::Shell);
                }
                if matches!(a.run, Run::Own(Own::EditHost)) {
                    return Step::Hand(Hand::Edit(PathBuf::from(papers()[0].path)));
                }
                if Self::arms(a) && armed != Some(a.id) {
                    self.pending = Some(a.id);
                    return Step::Draw;
                }
                Step::Run(a)
            }
        }
    }
}

/// The keys the console answers, named by what they mean and not by which key they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Across the split.
    Sideways,
    /// Run, open, or cross into the content.
    Enter,
    /// Close the console.
    Leave,
    /// Straight to a section — what a mouse click on the sidebar becomes.
    Pick(Section),
}

/// The system's generations, newest first, as `(number, when)`.
///
/// Read straight from the profile directory: no privilege, no `nix` invocation,
/// and it answers on a machine whose store is otherwise unusable.
#[must_use]
pub fn generations(profiles: &Path) -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir(profiles) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let number = name
                .strip_prefix("system-")?
                .strip_suffix("-link")?
                .parse::<u32>()
                .ok()?;
            let when = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map_or_else(String::new, |t| {
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    secs.to_string()
                });
            Some((number, when))
        })
        .collect();
    out.sort_unstable_by_key(|(n, _)| std::cmp::Reverse(*n));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tier;

    #[test]
    fn every_row_of_the_table_lands_in_a_section_of_the_sidebar() {
        // THE DEFECT THIS CATCHES: a row added to the table with a section the
        // sidebar does not offer is a fix that exists, is translated, is
        // checked — and that nobody can reach.
        for a in crate::all_of() {
            assert!(
                Section::all_of().contains(&a.section),
                "`{}` esta numa secao que a barra lateral nao mostra",
                a.id
            );
        }
    }

    #[test]
    fn every_section_of_the_sidebar_has_something_to_show() {
        for s in Section::all_of() {
            let has_rows = !of_section(s).is_empty();
            let has_own = matches!(s, Section::State | Section::Config | Section::Log);
            assert!(has_rows || has_own, "a secao {s:?} abre vazia");
        }
    }

    #[test]
    fn what_cannot_be_taken_back_asks_twice() {
        let mut v = View::fresh();
        v.section = Section::System;
        v.on_sidebar = false;
        let reboot = of_section(Section::System)
            .into_iter()
            .find(|a| a.id == "reboot")
            .expect("reiniciar");
        v.row = of_section(Section::System)
            .iter()
            .position(|a| a.id == "reboot")
            .expect("a posicao");

        // First press arms, and runs NOTHING.
        assert_eq!(v.key(Key::Enter), Step::Draw);
        assert_eq!(v.pending, Some("reboot"));
        // The same key again fires.
        assert_eq!(v.key(Key::Enter), Step::Run(reboot));
    }

    #[test]
    fn any_other_key_disarms() {
        // THE DEFECT: arming and walking away leaves a machine one Enter from
        // rebooting, and the screen that says so is two sections back.
        let mut v = View::fresh();
        v.section = Section::System;
        v.on_sidebar = false;
        v.row = of_section(Section::System)
            .iter()
            .position(|a| a.id == "poweroff")
            .expect("a posicao");
        assert_eq!(v.key(Key::Enter), Step::Draw);
        assert_eq!(v.pending, Some("poweroff"));
        let _ = v.key(Key::Down);
        assert_eq!(v.pending, None, "a seta deixou a maquina armada");
    }

    #[test]
    fn a_generated_file_opens_to_be_read_and_not_written() {
        let mut v = View::fresh();
        v.section = Section::Config;
        v.on_sidebar = false;
        v.row = papers()
            .iter()
            .position(|p| p.origin.is_some())
            .expect("um gerado");
        let Step::Hand(Hand::Read(_)) = v.key(Key::Enter) else {
            panic!("um arquivo gerado abriu para edicao — some no proximo rebuild");
        };
    }

    #[test]
    fn the_machines_own_file_opens_to_be_written() {
        let mut v = View::fresh();
        v.section = Section::Config;
        v.on_sidebar = false;
        v.row = 0;
        assert_eq!(papers()[0].path, "/etc/nixos/host.nix");
        let Step::Hand(Hand::Edit(_)) = v.key(Key::Enter) else {
            panic!("o host.nix abriu em leitura");
        };
    }

    #[test]
    fn the_sidebar_wraps_around_instead_of_stopping() {
        let mut v = View::fresh();
        assert!(v.on_sidebar);
        let _ = v.key(Key::Up);
        assert_eq!(
            v.section,
            Section::System,
            "subir na primeira devia dar a ultima"
        );
    }

    #[test]
    fn a_click_on_the_sidebar_goes_straight_to_the_content() {
        let mut v = View::fresh();
        let _ = v.key(Key::Pick(Section::Windows));
        assert_eq!(v.section, Section::Windows);
        assert!(!v.on_sidebar, "o clique parou na barra em vez de entrar");
    }

    #[test]
    fn a_free_row_never_arms() {
        // Arming is for what cannot be undone. Restarting the footer twice is
        // restarting the footer.
        for a in crate::all_of().iter().filter(|a| a.tier == Tier::Free) {
            assert!(!View::arms(a), "`{}` pede confirmacao a toa", a.id);
        }
    }

    #[test]
    fn the_generations_come_newest_first() {
        let dir = std::env::temp_dir().join(format!("rescue-generations-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("o diretorio");
        for n in [7_u32, 9, 8] {
            std::fs::write(dir.join(format!("system-{n}-link")), b"").expect("a geracao");
        }
        std::fs::write(dir.join("system-profile"), b"").expect("o que nao e geracao");
        let got: Vec<u32> = generations(&dir).into_iter().map(|(n, _)| n).collect();
        assert_eq!(got, vec![9, 8, 7]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
