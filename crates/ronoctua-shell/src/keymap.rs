//! The keymap the binary assembles: every crate's keys in one list — the
//! one place that names them all (shell-data-design §1.6) — and the tests
//! that ask the whole tree.
//!
//! # Why here
//!
//! Each crate says its own keys beside its view
//! ([`ronoctua_shell_kit::bindings::Contribution`]); the footer says the window's, the
//! workspace's and its own ([`ronoctua_shell_footer::leader::BASE`]). Something has to name
//! them all to build the tree the leader walks, and that is the binary's
//! business, as it names every door: the footer and the views never name a
//! feature. `main` installs [`build`] before any door is up
//! (`bindings::install`), and every door then asks the installed keymap.
//!
//! The tests that assert the REAL tree live here for the same reason: only
//! the binary has it.

use ronoctua_shell_kit::bindings::{Contribution, Trees};

/// Every crate's keys, the footer's first and then by crate, one a line.
///
/// A crate left out is a crate whose commands do not exist: its keys lead
/// nowhere and its rows run "does nothing yet" — `every-contribution-is-assembled`
/// reads every `KEYS` the shell declares against this list.
pub const CONTRIBUTIONS: &[&Contribution] = &[
    &ronoctua_shell_footer::leader::BASE,
    &ronoctua_shell_agents::agents::KEYS,
    &ronoctua_shell_audio::audio::KEYS,
    &ronoctua_shell_belongings::belongings::KEYS,
    &ronoctua_shell_bluetooth::bluetooth::KEYS,
    &ronoctua_shell_calc::calc::KEYS,
    &ronoctua_shell_camera::camera::KEYS,
    &ronoctua_shell_clipboard::clipboard_view::KEYS,
    &ronoctua_shell_containers::containers::KEYS,
    &ronoctua_shell_display::display::KEYS,
    &ronoctua_shell_fast_access::fast_access::KEYS,
    &ronoctua_shell_files::files::KEYS,
    &ronoctua_shell_git::dev_clone::KEYS,
    &ronoctua_shell_history::history::KEYS,
    &ronoctua_shell_icons::icons::KEYS,
    &ronoctua_shell_incus::incus::KEYS,
    &ronoctua_shell_locale::locale::KEYS,
    &ronoctua_shell_manual::manual::KEYS,
    &ronoctua_shell_network::network::KEYS,
    &ronoctua_shell_notifications::notifications::KEYS,
    &ronoctua_shell_overlays::capture::KEYS,
    &ronoctua_shell_player::player::KEYS,
    &ronoctua_shell_pointers::pointers::KEYS,
    &ronoctua_shell_power::power::KEYS,
    &ronoctua_shell_security::security::KEYS,
    &ronoctua_shell_software::software::KEYS,
    &ronoctua_shell_stick::stick::KEYS,
    &ronoctua_shell_syncthing::syncthing::KEYS,
    &ronoctua_shell_transfers::transfers::KEYS,
    &ronoctua_shell_update::update_window::KEYS,
    &ronoctua_shell_video_metrics::video_metrics::KEYS,
];

/// The keymap of this binary: every contribution, assembled
/// ([`ronoctua_shell_kit::bindings::assemble`]) — what `main` installs.
///
/// # Errors
///
/// A programming error in some crate's keys, named by `assemble`.
pub fn build() -> anyhow::Result<(Trees, ronoctua_cmd::Registry)> {
    ronoctua_shell_kit::bindings::assemble(CONTRIBUTIONS)
}

/// Every command some key of this binary reaches
/// ([`ronoctua_shell_kit::bindings::reachable`]).
#[must_use]
pub fn reachable(tree: &ronoctua_keymap::Tree) -> std::collections::BTreeSet<&'static str> {
    ronoctua_shell_kit::bindings::reachable(tree, CONTRIBUTIONS)
}

/// Every command reachable by name and by name only
/// ([`ronoctua_shell_kit::bindings::by_name_only`]).
#[must_use]
pub fn by_name_only() -> Vec<&'static str> {
    ronoctua_shell_kit::bindings::by_name_only(CONTRIBUTIONS)
}

#[cfg(test)]
mod tests {
    use super::{build, by_name_only, reachable};
    use crate::key;
    use ronoctua_keymap::{Key, Sym, parse_sequence};
    use ronoctua_shell_footer::leader::{
        APPS_KEYS, BRAIN_KEYS, EVERY_WORKSPACE_KEYS, Effect, GLANCE_KEYS, Leader, OVERVIEW_KEYS,
        RANGE_ROW, a_module_action, condensed_root,
    };
    use ronoctua_shell_kit::bindings::{Set, label, taught_keys, taught_keys_with};

    /// When the CASE is the reason, the message has to say so.
    ///
    /// `CTRL ALT SPC I is not bound` is true and is useless: it shows the
    /// consequence and hides the cause, and sends the person looking for a shortcut
    /// they have just pressed. Whoever pressed Shift by accident only finds out
    /// if the screen tells them that the other case is the one that exists.
    #[test]
    fn the_message_points_at_the_other_case() {
        let (tree, _) = build().expect("the default keymap has to close");
        let mut l = Leader::default();
        l.activate();
        // `M` and not `I`: the letter is only an EXAMPLE of a first-level
        // binding, and the icons moved under `m` when `m` was made the drawer
        // for what belongs nowhere else.
        match l.key(&tree, Key::simple(Sym::Char('M'))) {
            Effect::Close {
                other_case: Some(t),
            } => assert_eq!(
                t.sym,
                Sym::Char('m'),
                "the suggestion has to be the lower-case one"
            ),
            otherwise => panic!("`M` should end pointing at `m`; got {otherwise:?}"),
        }
    }

    #[test]
    fn with_no_other_case_there_is_no_suggestion() {
        // The suggestion must not become noise: a key that exists in no case at
        // all ends without pointing at anything, and one that HAS no case (a digit,
        // punctuation) too.
        let (tree, _) = build().expect("the default keymap has to close");
        // `¬` AND NOT `=`, which the calculator took (`LEADER =`).
        for c in ['ç', '¬'] {
            let mut l = Leader::default();
            l.activate();
            assert!(
                matches!(
                    l.key(&tree, Key::simple(Sym::Char(c))),
                    Effect::Close { other_case: None }
                ),
                "`{c}` exists in no case at all; there is nothing to suggest"
            );
        }
    }

    /// `CTRL ALT SPC M-n is not bound` — the owner's second screen.
    ///
    /// The `M-` is Alt. In the nested loop the leader **is** `Ctrl+Alt+Space`, so Ctrl
    /// and Alt are held down by construction when the sequence begins; and after
    /// a host shortcut (`Super+Space`) the nested compositor may miss the key's
    /// release and announce it forever. In both cases every binding
    /// stops matching at once.
    #[test]
    fn the_message_points_at_the_key_with_no_modifier() {
        use ronoctua_keymap::Mods;
        let (tree, _) = build().expect("the default keymap has to close");
        let with_alt = Key {
            mods: Mods {
                ctrl: false,
                alt: true,
                shift: false,
                logo: false,
            },
            sym: Sym::Char('m'),
        };
        // The premise: `M-m` is not bound to anything, and `m` is. The letter
        // is only an example of a first-level binding; the icons' `i` was it
        // until `m` became the drawer they moved into.
        assert!(matches!(
            tree.footer.resolve(&[with_alt]),
            ronoctua_keymap::Resolution::Nothing
        ));

        let mut l = Leader::default();
        l.activate();
        match l.key(&tree, with_alt) {
            Effect::Close {
                other_case: Some(t),
            } => {
                assert_eq!(t.sym, Sym::Char('m'));
                assert!(
                    !t.mods.alt,
                    "the suggestion has to be the key WITHOUT the Alt"
                );
            }
            otherwise => panic!("`M-m` should end pointing at `m`; got {otherwise:?}"),
        }
    }

    #[test]
    fn the_other_case_runs_nothing() {
        // THE SUGGESTION IS NOT A FALLBACK. `w H` moves the window and `w h` focuses: falling
        // from one to the other would make the shortcut do SOMETHING ELSE, which is worse than
        // doing nothing. Here `w` followed by a non-existent key ends the sequence —
        // never `Run`.
        let (tree, _) = build().expect("the default keymap has to close");
        let mut l = Leader::default();
        l.activate();
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('w'))),
            Effect::Continue { .. }
        ));
        // `T` is not in `w`, but `t` is (`window-float`).
        assert!(
            matches!(
                l.key(&tree, Key::simple(Sym::Char('T'))),
                Effect::Close {
                    other_case: Some(_)
                }
            ),
            "it has to end pointing at `t`, and NOT run `window-float`"
        );
    }

    #[test]
    fn the_default_keymap_does_not_collide() {
        // This is milestone 5's "keymap collision check". It runs in
        // `nix flake check` through `rust-tests`, and also through the
        // `check-keymap` subcommand, which exists for whoever edits their own keymap.
        // It asserts the INVARIANT, and not a fixed count: a hard-coded number
        // breaks on every new command and teaches people to update the test without reading
        // what it says — which is how a check stops verifying.
        let (tree, registry) = build().expect("the default keymap has to close");

        assert!(tree.full.amount() > 0, "an empty keymap");

        // NO ORPHAN COMMAND: every command in the registry has SOME key path
        // to it.
        //
        // "Path" used to be only the sequence. It is not any more: a command can be
        // reached by SEQUENCE (`LEADER w h`), by CHORD (`LEADER+←`, which
        // the compositor resolves and the keymap never sees) or by EMERGENCY
        // BIND (`Ctrl+Shift+L`, which exists outside the leader precisely
        // for when it does not respond). Counting only the first would flag an orphan
        // where there is a shortcut, and the fix would be inventing a sequence
        // nobody asked for.
        //
        // What still holds, and is what matters: no command with no
        // path at all. Being reachable by `M-x` alone is not enough — a command that only
        // gets typed is a command only whoever read the code knows exists.
        let reachable = reachable(&tree.full);
        let by_name = by_name_only();
        for c in registry.all_ones() {
            assert!(!c.id.is_empty(), "a command with no id");
            assert!(!c.label.is_empty(), "a command with no label key");
            // THE COMPOSITOR'S DEEDS ARE THE EXCEPTION, and the only one.
            //
            // They exist to be reachable by NAME (section 13); giving keys to
            // all of them would spend the whole keymap on what is almost never
            // used. The exception is recognisable by its prefix, so it cannot
            // grow without someone writing `wm-` on purpose.
            if c.id.starts_with("wm-") || by_name.contains(&c.id) || a_module_action(c.id) {
                continue;
            }
            assert!(
                reachable.contains(&c.id),
                "`{}` is OUR command and no key reaches it: \
             no sequence, no chord, no emergency bind",
                c.id
            );
        }

        // And the converse: nothing points at a command that does not exist. `build()`
        // already refuses to build in that case; this proves the refusal is wired up.
        for id in reachable {
            assert!(
                registry.search(id).is_some(),
                "there is a key pointing at `{id}`, which is not in the registry"
            );
        }
    }

    #[test]
    fn esc_always_ends_it() {
        // Even with the sequence half-typed. Without an unconditional exit, a
        // lost user has no way back.
        let (tree, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        // `s` and not `b`: `b` was a prefix until `LEADER b` became the brain
        // itself, and a command ENDS the sequence, which is the opposite of
        // what this is testing.
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('s'))),
            Effect::Continue { .. }
        ));
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Esc)),
            Effect::Close { .. }
        ));
        assert!(!l.active_one());
    }

    #[test]
    fn a_key_with_no_binding_ends_instead_of_hanging() {
        let (tree, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('ç'))),
            Effect::Close { .. }
        ));
        assert!(!l.active_one());
    }

    #[test]
    fn a_complete_sequence_runs_and_leaves() {
        let (tree, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        // `s q h` — THREE keys, which is also what this test wants: a sequence
        // that runs has to be walked to the end, and a two-key one cannot tell
        // "the last key ran it" from "any key after a group runs it". `s q`
        // has one door, so it does not fold in the set a leader starts in.
        l.key(&tree, Key::simple(Sym::Char('s')));
        l.key(&tree, Key::simple(Sym::Char('q')));
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('h'))),
            Effect::Run("power-hibernate", None)
        );
        // Leaving the leader after executing is what gives the keyboard back to the
        // buffers. Staying active would leave the system with no input.
        assert!(!l.active_one());
    }

    /// FAILS IF: the file manager stops being a submenu of `.` in the full
    /// tree, stops being `.` itself in the other two, or `f` comes back.
    ///
    /// The owner asked for the shape in as many words — *"não quero que
    /// `LEADER f` exista; quero que `LEADER .` seja um submenu; `LEADER . .`
    /// deve abrir o filemanager no rodapé; e `LEADER . w` deve abrir a versão
    /// janela"*. Before it, one thing was reachable by `.`, by `f f` and — the
    /// window — by `f w`.
    ///
    /// AND THEN FOR THE THREE SETS (2026-09-28): the submenu is still there,
    /// whole, in the full set (the leader pressed twice more); the leader
    /// alone runs the footer's door on `.`, and pressed again the same `.` is
    /// the window. In the full
    /// tree `.` ALONE still runs nothing — a prefix that also executed would
    /// run a command while somebody is still typing, which is the rule the
    /// keymap refuses in writing.
    #[test]
    fn the_manager_is_a_submenu_of_the_dot() {
        let (trees, _) = build().unwrap();
        let walk = |set: Set, seq: &[char]| {
            let mut l = Leader::default();
            l.activate();
            assert!(l.set(set), "the set could not be chosen at the root");
            let mut last = Effect::Close { other_case: None };
            for c in seq {
                last = l.key(&trees, Key::simple(Sym::Char(*c)));
            }
            last
        };
        assert_eq!(
            walk(Set::Full, &['.', '.']),
            Effect::Run("files-open", None)
        );
        assert_eq!(
            walk(Set::Full, &['.', 'w']),
            Effect::Run("files-window", None)
        );
        // `.` ALONE IS A GROUP in the full tree, so it runs nothing there.
        assert!(
            !matches!(walk(Set::Full, &['.']), Effect::Run(..)),
            "`.` executes in the full tree, so it cannot honestly be a prefix there"
        );
        // AND IS EACH DOOR in the other two.
        assert_eq!(walk(Set::Footer, &['.']), Effect::Run("files-open", None));
        assert_eq!(walk(Set::Window, &['.']), Effect::Run("files-window", None));
        // `f` LEADS NOWHERE, in any set — which is what "I do not want it to
        // exist" means: not a group with nothing in it, not a key that runs
        // the manager by another road.
        for set in Set::ALL {
            assert!(
                !matches!(walk(set, &['f']), Effect::Run(..)),
                "`f` runs something in the {set:?} set"
            );
            for c in ['f', 'w'] {
                assert!(
                    !matches!(walk(set, &['f', c]), Effect::Run(..)),
                    "`f {c}` still reaches a command in the {set:?} set"
                );
            }
        }
    }

    #[test]
    fn the_numbers_carry_the_workspace_in_the_argument() {
        // NINE KEYS, ONE COMMAND. The alternative would be `workspace-ir-1`…`-9`:
        // nine names in the locator saying the same thing and nine text
        // keys to translate, just to carry a number.
        let (tree, _) = build().unwrap();
        for n in 1..=9u8 {
            let mut l = Leader::default();
            l.activate();
            l.key(&tree, Key::simple(Sym::Tab));
            let d = char::from_digit(u32::from(n), 10).unwrap();
            match l.key(&tree, Key::simple(Sym::Char(d))) {
                Effect::Run(id, Some(arg)) => {
                    assert_eq!(id, "workspace-go");
                    assert_eq!(
                        arg.parse::<u8>().unwrap(),
                        n,
                        "the key and the argument diverged"
                    );
                }
                otherwise => panic!("`TAB {d}` did not lead to workspace {n}: {otherwise:?}"),
            }
        }
    }

    /// FAILS IF: `LEADER 1`…`9` stop going to their workspace, or `LEADER 0`
    /// stops opening the overview — the keys the modeline's numbers teach.
    #[test]
    fn a_digit_at_the_top_goes_to_its_workspace_and_zero_shows_them_all() {
        let (tree, _) = build().unwrap();
        for n in 1..=9u32 {
            let mut l = Leader::default();
            l.activate();
            let d = char::from_digit(n, 10).unwrap();
            match l.key(&tree, Key::simple(Sym::Char(d))) {
                Effect::Run("workspace-go", Some(arg)) => {
                    assert_eq!(arg, n.to_string(), "`LEADER {d}`");
                }
                otherwise => panic!("`LEADER {d}` did not go to workspace {n}: {otherwise:?}"),
            }
        }
        let mut l = Leader::default();
        l.activate();
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('0'))),
            Effect::Run("workspace-all", None)
        );
    }

    /// FAILS IF: the keys the line under the modeline prints to leave an
    /// overview stop being the keys that do it — in ANY of the three sets:
    /// the line prints them after the leader alone, and `TAB` folds in none.
    #[test]
    fn the_keys_printed_to_leave_an_overview_are_bound() {
        let (trees, _) = build().unwrap();
        for set in Set::ALL {
            for (keys, wanted) in [
                (EVERY_WORKSPACE_KEYS, "workspace-all"),
                (OVERVIEW_KEYS, "workspace-overview"),
                ("TAB a", "workspace-all"),
            ] {
                let seq = parse_sequence(keys).unwrap();
                assert!(
                    matches!(trees.of(set).resolve(&seq), ronoctua_keymap::Resolution::Command(id, None) if id == wanted),
                    "`{keys}` is not `{wanted}` in the {set:?} set"
                );
            }
        }
    }

    /// FAILS IF: `LEADER /` stops being the development group with the
    /// resizer at `/ r`, the development group holds `d` again — the fast
    /// access's (2026-09-30) — or the glance leaves `g`, or the constant its
    /// sentences teach it by stops being the key that runs it.
    #[test]
    fn the_dev_group_is_slash_and_the_glance_is_g() {
        // THE DEFAULT SET, which is where a sequence starts; `/` folds in none.
        let (trees, _) = build().unwrap();
        let tree = &trees.footer;
        let runs = |keys: &str| match tree.resolve(&parse_sequence(keys).unwrap()) {
            ronoctua_keymap::Resolution::Command(id, None) => Some(id),
            _ => None,
        };
        assert_eq!(runs("/ r"), Some("dev-resizer"));
        assert_eq!(runs("g"), Some("world-glance"));
        assert_eq!(GLANCE_KEYS, "g");
        assert_eq!(runs(GLANCE_KEYS), Some("world-glance"));
        assert_eq!(runs("/"), None, "`/` runs something, so it is no group");
        let ronoctua_keymap::Resolution::Prefix(root) = tree.resolve(&[]) else {
            panic!("the root is not a list");
        };
        let dev = root
            .iter()
            .find(|e| e.key == "/")
            .expect("no `/` at the root");
        assert_eq!(dev.command, None, "`/` is a command and not a group");
        assert_eq!(dev.label, key::GROUP_DEV);
        // `d` IS NOT THE DEVELOPMENT GROUP'S ANY MORE, in any set: whatever
        // the fast access binds there, the resizer is not under it.
        for set in Set::ALL {
            let t = trees.of(set);
            assert!(
                !matches!(
                    t.resolve(&parse_sequence("d r").unwrap()),
                    ronoctua_keymap::Resolution::Command("dev-resizer", _)
                ),
                "`d r` still runs the resizer in the {set:?} set"
            );
            let ronoctua_keymap::Resolution::Prefix(root) = t.resolve(&[]) else {
                panic!("the root of the {set:?} set is not a list");
            };
            assert!(
                !root
                    .iter()
                    .any(|e| e.key == "d" && e.label == key::GROUP_DEV),
                "`d` is still the development group in the {set:?} set"
            );
        }
    }

    /// FAILS IF: the root list draws the nine digits as nine rows again, or
    /// the condensed row's click stops opening the workspace group.
    #[test]
    fn the_nine_digits_are_one_row_whose_click_opens_the_workspaces() {
        let (tree, _) = build().unwrap();
        let ronoctua_keymap::Resolution::Prefix(root) = tree.footer.resolve(&[]) else {
            panic!("the root is not a list");
        };
        let shown = condensed_root(root.clone());
        assert_eq!(root.len() - shown.len(), 8, "nine rows became one");
        let row = shown
            .iter()
            .find(|e| e.key == RANGE_ROW)
            .expect("no `1…9` row");
        assert_eq!(row.command.as_deref(), Some("workspace-go"));
        assert!(!shown.iter().any(|e| e.key == "3"));
        // `0` is another thing — every workspace at once — and keeps its row.
        assert!(shown.iter().any(|e| e.key == "0"));
        // THE CLICK PRESSES `sym`, and `sym` opens the group where each
        // workspace is a row of its own.
        let mut l = Leader::default();
        l.activate();
        match l.key(&tree, row.sym) {
            Effect::Continue { inputs } => {
                assert!(
                    inputs
                        .iter()
                        .any(|e| e.key == "3" && e.command.as_deref() == Some("workspace-go"))
                );
            }
            otherwise => panic!("the `1…9` row did not open the workspaces: {otherwise:?}"),
        }
    }

    #[test]
    fn the_zero_goes_to_the_last_workspace() {
        // The keyboard's convention: zero closes the row after nine. With
        // nine fixed workspaces in the compositor, the last is the ninth.
        let (tree, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        l.key(&tree, Key::simple(Sym::Tab));
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('0'))),
            Effect::Run("workspace-go", Some("9"))
        );
    }

    #[test]
    fn moving_the_window_has_the_same_keys_as_going() {
        // THE REQUEST: whoever learned `TAB 3` gets `TAB m 3` for free. A
        // key that exists in one submenu and is missing from the other is an exception one only
        // discovers in use.
        let (tree, _) = build().unwrap();
        let ir = |seq: &[Key]| {
            let mut l = Leader::default();
            l.activate();
            let mut last = Effect::Close { other_case: None };
            for t in seq {
                last = l.key(&tree, *t);
            }
            last
        };
        let tab = Key::simple(Sym::Tab);
        let m = Key::simple(Sym::Char('m'));
        for c in ['j', 'k', '[', ']', '.', ',', '0', '1', '9'] {
            let key = Key::simple(Sym::Char(c));
            assert!(
                matches!(ir(&[tab, key]), Effect::Run(..)),
                "`TAB {c}` leads to no command at all"
            );
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "`TAB m {c}` leads to no command at all"
            );
        }
        // And the arrows, in both.
        for sym in [Sym::Up, Sym::Down] {
            let key = Key::simple(sym);
            assert!(
                matches!(ir(&[tab, key]), Effect::Run(..)),
                "{sym:?} after TAB"
            );
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "{sym:?} after TAB m"
            );
        }
    }

    #[test]
    fn the_prefix_is_dooms_notation() {
        let (tree, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert_eq!(l.prefix("SPC"), "SPC-");
        assert_eq!(l.raw_prefix("SPC"), "SPC");
        // `s` and not `b`, for the reason `esc_always_ends_it` gives: `b` is a
        // command now and a command ends the sequence.
        l.key(&tree, Key::simple(Sym::Char('s')));
        assert_eq!(l.prefix("SPC"), "SPC s-");
        // Without the ellipsis: in an error message it would lie, because
        // there the sequence has ended and there is no continuation.
        assert_eq!(l.raw_prefix("SPC"), "SPC s");
    }

    /// FAILS IF: the chord on screen stops saying which set the next key
    /// walks — as the leader pressed again, `SUPER SUPER-`, `SUPER SUPER SUPER
    /// s n-` — or the default set grows a word of its own.
    #[test]
    fn the_prefix_names_the_set() {
        let (trees, _) = build().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.prefix("SUPER"), "SUPER SUPER-");
        assert_eq!(l.raw_prefix("SUPER"), "SUPER SUPER");
        // THE LAST SWITCH WINS, while nothing has been typed.
        assert!(l.set(Set::Full));
        l.key(&trees, Key::simple(Sym::Char('s')));
        l.key(&trees, Key::simple(Sym::Char('n')));
        assert_eq!(l.prefix("SUPER"), "SUPER SUPER SUPER s n-");
        assert_eq!(l.raw_prefix("SUPER"), "SUPER SUPER SUPER s n");
        // AND THE NESTED LOOP'S LEADER, three keys long, is repeated whole.
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.prefix("CTRL ALT SPC"), "CTRL ALT SPC CTRL ALT SPC-");
        // AND THE DEFAULT IS THE LEADER ALONE, from every opening.
        l.activate();
        assert_eq!(l.set_now(), Set::Footer);
        assert_eq!(l.prefix("SUPER"), "SUPER-");
    }

    /// FAILS IF: the set can be switched anywhere but the root of an open
    /// leader — halfway through `s n` the keys already typed would change
    /// meaning under the hand that typed them.
    #[test]
    fn the_set_switches_only_at_the_root() {
        let (trees, _) = build().unwrap();
        let mut l = Leader::default();
        assert!(!l.set(Set::Window), "a closed leader took a set");
        assert_eq!(l.set_now(), Set::Footer);
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.set_now(), Set::Window);
        assert!(matches!(
            l.key(&trees, Key::simple(Sym::Char('s'))),
            Effect::Continue { .. }
        ));
        assert!(!l.set(Set::Full), "the set switched with `s` typed");
        assert_eq!(l.set_now(), Set::Window);
        // AND IT IS WHAT THE NEXT KEY WALKS: `s n` in the window set is the
        // network's window.
        assert_eq!(
            l.key(&trees, Key::simple(Sym::Char('n'))),
            Effect::Run("network-window", None)
        );
        // ENDING THE SEQUENCE puts the default back.
        assert_eq!(l.set_now(), Set::Footer);
    }

    /// FAILS IF: a group folds that did not, or stops folding — the rule's
    /// answer over the real tables, pinned. A group added tomorrow that keeps
    /// the two-door convention lands here on purpose, and whoever adds it
    /// reads why: `LEADER` alone then runs its footer door and `LEADER
    /// LEADER` its window.
    ///
    /// `w` is NOT here (its doubled key is its `w`), nor any level of the
    /// window menu under it (`w v` has `w v v` and `w v w`, two deeds and no
    /// doors), nor `RET`, `TAB`, `TAB m` or `s q` (one door each), nor `m`,
    /// `m i`, `s`, `d` or `t i` (no door of their own).
    #[test]
    fn the_collapsed_groups_are_these() {
        use ronoctua_keymap::Resolution;
        let (trees, _) = build().unwrap();
        // EVERY GROUP OF THE FULL TREE, walked from the root.
        let mut groups: Vec<Vec<Key>> = Vec::new();
        let mut edge: Vec<Vec<Key>> = vec![Vec::new()];
        while let Some(at) = edge.pop() {
            let Resolution::Prefix(entries) = trees.full.resolve(&at) else {
                continue;
            };
            for e in entries.iter().filter(|e| e.is_group) {
                let mut seq = at.clone();
                seq.push(e.sym);
                groups.push(seq.clone());
                edge.push(seq);
            }
        }
        let command = |set: Set, seq: &[Key]| match trees.of(set).resolve(seq) {
            Resolution::Command(id, None) => Some(id),
            _ => None,
        };
        let mut folded: Vec<(String, &str, &str)> = groups
            .iter()
            .filter_map(|g| {
                Some((
                    ronoctua_keymap::notation(g),
                    command(Set::Footer, g)?,
                    command(Set::Window, g)?,
                ))
            })
            .collect();
        folded.sort_unstable();
        let mut wanted: Vec<(String, &str, &str)> = [
            (".", "files-open", "files-window"),
            ("p", "player-open", "player-window"),
            ("t", "transfers-open", "transfers-window"),
            ("n", "notif-open", "notif-window"),
            ("i", "software-open", "software-window"),
            ("s h", "atuin-open", "atuin-window"),
            ("s n", "network-open", "network-window"),
            ("s s", "syncthing-open", "syncthing-window"),
            ("s d", "display-open", "display-window"),
            ("s a", "audio-open", "audio-window"),
            ("s c", "camera-settings-open", "camera-settings-window"),
            ("s i", "pointers-open", "pointers-window"),
            ("s p", "power-open", "power-window"),
            ("s b", "bluetooth-open", "bluetooth-window"),
            ("s l", "locale-open", "locale-window"),
            ("s g", "geo-open", "geo-window"),
            (
                "s u",
                ronoctua_shell_update::update_window::OPEN,
                ronoctua_shell_update::update_window::WATCH,
            ),
            ("s r", "footer-settings-open", "footer-settings-window"),
            ("m i", "icons-open", "icons-window"),
            ("=", "calc-open", "calc-window"),
            ("v", "video-metrics-open", "video-metrics-window"),
            ("s e", "security-open", "security-window"),
            ("s o", "containers-open", "containers-window"),
            ("s v", "belongings-open", "belongings-window"),
            ("d", "fast-access-open", "fast-access-window"),
            ("c", "clipboard-open", "clipboard-window"),
            ("s z", "stick-open", "stick-window"),
            ("s x", "incus-open", "incus-window"),
            ("m a", "agents-open", "agents-window"),
        ]
        .into_iter()
        .map(|(g, f, w)| (g.to_owned(), f, w))
        .collect();
        wanted.sort_unstable();
        assert_eq!(folded, wanted);
        // A GROUP FOLDS IN BOTH SETS OR IN NEITHER: a key that is a command
        // in one and a list in the other would be a third rule nobody wrote.
        for g in &groups {
            assert_eq!(
                command(Set::Footer, g).is_some(),
                command(Set::Window, g).is_some(),
                "`{}` folds in one set only",
                ronoctua_keymap::notation(g)
            );
        }
    }

    /// FAILS IF: the full set stops being the whole tree, or a folded key
    /// keeps its submenu in the default set.
    #[test]
    fn the_three_trees_are_the_three_sets() {
        use ronoctua_keymap::Resolution;
        let (trees, _) = build().unwrap();
        let resolve = |set: Set, keys: &str| trees.of(set).resolve(&parse_sequence(keys).unwrap());
        assert_eq!(
            resolve(Set::Footer, "s n"),
            Resolution::Command("network-open", None)
        );
        assert_eq!(
            resolve(Set::Window, "s n"),
            Resolution::Command("network-window", None)
        );
        assert!(matches!(resolve(Set::Full, "s n"), Resolution::Prefix(_)));
        // THE REST OF A FOLDED GROUP is two presses of the leader away, and
        // only there.
        assert_eq!(
            resolve(Set::Full, "s n a"),
            Resolution::Command("network-airplane", None)
        );
        assert_eq!(resolve(Set::Footer, "s n a"), Resolution::Nothing);
        // WHAT DOES NOT FOLD IS EVERYWHERE — the window's own letter included.
        for set in Set::ALL {
            assert_eq!(
                resolve(set, "w f"),
                Resolution::Command("window-screen-full", None)
            );
            assert_eq!(
                resolve(set, "RET RET"),
                Resolution::Command("buffer-terminal-external", None)
            );
            assert!(matches!(resolve(set, "s"), Resolution::Prefix(_)));
        }
        // THE FULL SET IS STRICTLY BIGGER: it lost nothing to the folding.
        assert!(trees.full.amount() > trees.footer.amount());
        assert!(trees.full.amount() > trees.window.amount());
    }

    /// FAILS IF: a sentence would teach a key that does something else when
    /// typed from where every sequence starts.
    #[test]
    fn the_taught_keys_start_in_the_default_set() {
        let (trees, _) = build().unwrap();
        let taught = |id: &str| taught_keys_with(&trees, id, None, "SUPER");
        // THE FOOTER'S OWN SEQUENCE, when it has one.
        assert_eq!(taught("transfers-open").as_deref(), Some("t"));
        assert_eq!(taught("notif-open").as_deref(), Some("n"));
        assert_eq!(taught("window-screen-full").as_deref(), Some("w f"));
        assert_eq!(taught("apps-open").as_deref(), Some(APPS_KEYS));
        assert_eq!(taught("brain-open").as_deref(), Some(BRAIN_KEYS));
        // ELSE THE WINDOW SET'S, after the leader pressed again.
        assert_eq!(taught("files-window").as_deref(), Some("SUPER ."));
        // ELSE THE FULL TREE'S, after the leader pressed twice more.
        assert_eq!(
            taught("bluetooth-accept"),
            Some(format!(
                "SUPER SUPER {}",
                ronoctua_shell_bluetooth::bluetooth::PAIRING_YES_KEYS
            ))
        );
        assert_eq!(
            taught("software-trials"),
            Some(format!(
                "SUPER SUPER {}",
                ronoctua_shell_software::software::SOFTWARE_TRIALS_KEYS
            ))
        );
        // AND THE ARGUMENT IS PART OF THE COMMAND.
        assert_eq!(
            taught_keys_with(&trees, "workspace-go", Some("3"), "SUPER").as_deref(),
            Some("3")
        );
        assert_eq!(taught("mx-does-not-exist"), None);
        // AND THE PROCESS'S OWN LEADER IS WHAT `taught_keys` REPEATS.
        assert_eq!(
            taught_keys(&trees, "files-window", None),
            Some(format!("{} .", label()))
        );
    }

    #[test]
    fn every_binding_points_at_a_command_that_exists() {
        // The bug section 13 exists to prevent: a shortcut pointing at a
        // non-existent command. `build()` refuses to build in that case, so
        // this test is the proof that the refusal is wired up.
        let (_, registry) = build().unwrap();
        for id in [
            "buffer-next",
            "buffer-close",
            "buffer-terminal",
            "icons-open",
            "session-quit",
            "mx-open",
        ] {
            assert!(registry.search(id).is_some(), "the command {id} is missing");
        }
    }

    #[test]
    fn every_keymap_command_has_an_action_in_the_shell() {
        // THE DEFECT THIS PREVENTS: five commands appeared in which-key,
        // were "executed", and fell into a log `warn`. On screen: nothing — and
        // indistinguishable from an unregistered key.
        //
        // The list of ids with an action is in `surface::run_command`. It is
        // repeated here on purpose: a `match` is not enumerable at
        // runtime, and the alternative (a table consulted by both) would be
        // one more indirection for the same effect. What matters is that
        // adding a binding with no action BREAKS something.
        const WITH_ACTION: &[&str] = &[
            "workspace-all",
            "session-quit",
            "notif-open",
            "notif-window",
            "notif-read-all",
            "notif-dismiss",
            // The nine that reach the manager's focused row, through the same
            // `View::key` the keyboard uses — see `Footer::manager_press`.
            "notif-read",
            "notif-forget",
            "notif-act",
            "notif-adjust",
            "notif-up",
            "notif-down",
            "notif-pane-next",
            "notif-pane-previous",
            "notif-knob-next",
            "notif-see",
            "notif-filters",
            "notif-new",
            "notif-drop",
            "notif-share",
            "notif-share-answer",
            "notif-draft-type",
            "notif-draft-add",
            "notif-draft-drop",
            "notif-draft-save",
            "notif-knob-previous",
            "notif-knob-turn",
            "notif-restore",
            "display-open",
            "display-window",
            "display-up",
            "display-down",
            "display-less",
            "display-more",
            "display-enter",
            "display-automatic",
            "display-forget",
            "display-arrange",
            // SOUND: the two doors, the view's seven gestures, and the
            // keyboard's own top row. The last seven have no leader at all —
            // the compositor binds them bare — but they are commands like
            // everything else, so this list is where they are accounted for.
            "audio-open",
            "audio-window",
            "audio-up",
            "audio-down",
            "audio-enter",
            "audio-less",
            "audio-more",
            "audio-mute",
            "audio-default",
            "pointers-open",
            "pointers-window",
            "camera-open",
            "onscreen-keyboard",
            "gestures",
            "gestures-calibrate",
            "dev-resizer",
            "camera-settings-open",
            "camera-settings-window",
            "camera-settings-up",
            "camera-settings-down",
            "camera-settings-less",
            "camera-settings-more",
            "camera-settings-enter",
            "camera-settings-remove",
            "camera-photo",
            "camera-record",
            "camera-effect-next",
            "camera-effect-previous",
            "camera-background-next",
            "camera-param-less",
            "camera-param-more",
            "camera-camera-next",
            "camera-mirror",
            "camera-scene-next",
            "camera-scene-previous",
            "camera-script",
            "camera-next-cue",
            "camera-virtual",
            "camera-stream",
            "camera-scene-add",
            "camera-scene-rename",
            "camera-script-add",
            "camera-stream-url",
            "camera-stream-key",
            "player-open",
            "player-window",
            "player-column",
            "player-play-file",
            "player-enqueue",
            "player-together",
            "player-machines",
            "player-pair",
            "player-toggle",
            "player-next",
            "player-previous",
            "player-stop",
            "player-shuffle",
            "player-repeat",
            "player-up",
            "player-down",
            "player-less",
            "player-more",
            "player-enter",
            "player-remove",
            "player-browse",
            "player-add",
            "player-only",
            "player-maximize",
            "player-play-now",
            "player-clear",
            "player-quieter",
            "player-louder",
            "player-pip",
            "player-mute",
            "player-swap",
            "player-tracks",
            "player-fold",
            "player-earlier",
            "player-later",
            "player-also",
            "player-rename",
            "player-save-set",
            "player-save-queue",
            "player-quit",
            "player-subtitles",
            "player-lane-shown",
            "player-ambience",
            "player-ambience-silence",
            "player-timeline",
            "player-layout",
            "player-metadata",
            "player-suggestions",
            "player-find-subtitles",
            "player-find-lyrics",
            "player-internet",
            "player-every-screen",
            "player-radio",
            "player-tv",
            "player-channels",
            "player-search",
            "player-favourite",
            "player-nearer",
            "player-further",
            "player-world",
            "pointers-up",
            "pointers-down",
            "pointers-enter",
            "pointers-less",
            "pointers-more",
            "pointers-forget",
            "volume-up",
            "volume-down",
            "volume-mute",
            "mic-mute",
            "media-play",
            "media-next",
            "media-previous",
            "media-stop",
            "brightness-up",
            "brightness-down",
            "power-open",
            "power-window",
            "power-profile-performance",
            "power-profile-balanced",
            "power-profile-saver",
            "power-keep-awake",
            "power-keep-lid",
            "power-thresholds",
            "power-suspend",
            "power-hibernate",
            "power-reboot",
            "power-off",
            "power-enter",
            "power-less",
            "power-more",
            "power-up",
            "power-down",
            "world-glance",
            "locale-open",
            "locale-window",
            "geo-open",
            "geo-window",
            "geo-up",
            "geo-down",
            "geo-enter",
            "geo-find",
            "geo-remove",
            "locale-less",
            "locale-more",
            "locale-zone",
            "locale-place",
            "locale-where",
            "locale-enter",
            "locale-up",
            "locale-down",
            "bluetooth-more",
            "bluetooth-settings",
            "bluetooth-less",
            "bluetooth-open",
            "bluetooth-window",
            "bluetooth-scan",
            "bluetooth-power",
            "bluetooth-discoverable",
            "bluetooth-reconnect",
            "bluetooth-disconnect-all",
            "bluetooth-accept",
            "bluetooth-refuse",
            "bluetooth-device",
            "bluetooth-connect",
            "bluetooth-pair",
            "bluetooth-forget",
            "bluetooth-trust",
            "bluetooth-block",
            "bluetooth-rename",
            "bluetooth-unnamed",
            "bluetooth-up",
            "bluetooth-down",
            "keyboard-restore",
            "help",
            "mx-open",
            // THE LEADER'S THREE SETS: no sequence reaches them (the leader
            // is not a key of the tree), and the hint's click and `LEADER :`
            // run them through `run_with`.
            "leader-set-window",
            "leader-set-full",
            "apps-open",
            "buffer-switch",
            "files-open",
            "files-window",
            "buffer-terminal",
            "buffer-terminal-external",
            // THE CAPTURE IS OURS. The compositor takes none, so the shell
            // runs `grim` and builds the name — see `capture_the_screen`.
            "screen-capture",
            "monitor-btop",
            "rescue-open",
            "monitor-bottom",
            "keyboard-open",
            "memory-open",
            // THE BRAIN. `brain-open` is the screen; the other two answer the
            // countdown on the footer's own line, with no window open — which
            // is why they are commands of the registry and not keys of a view.
            "brain-open",
            "brain-abort",
            "brain-proceed",
            "buffer-close",
            "buffer-next",
            // THE TRANSFER QUEUE. Only `transfers-open` is a command of the
            // registry; everything that acts on a row is a key of the view,
            // like the file manager's.
            "transfers-open",
            "transfers-window",
            "format-open",
            "transfers-add",
            "transfers-video",
            "transfers-audio",
            "transfers-subtitles",
            "transfers-add-as-me",
            "transfers-video-as-me",
            "transfers-audio-as-me",
            "transfers-subtitles-as-me",
            // THE CLIPBOARD. `clipboard-open` has a key in the tree; the other
            // six have keys INSIDE the view, and that is why they go through
            // `reachable` — all seven need an action, otherwise they would appear in the
            // list and do nothing.
            "apps-pin",
            "apps-details",
            "apps-remove",
            "software-open",
            "software-window",
            "software-trials",
            "software-maintenance",
            "software-android",
            "software-web-new",
            "software-web-address",
            "software-web-name",
            "software-icon-path",
            "software-file-path",
            "software-launch-latest",
            "software-down",
            "software-up",
            "software-enter",
            "software-next-source",
            "software-previous-source",
            "software-less",
            "software-more",
            "software-install",
            "software-try-it",
            "software-launch",
            "software-update",
            "software-edit",
            "software-remove",
            "software-stop",
            "software-paste",
            "clipboard-open",
            "clipboard-copy-of-new",
            "clipboard-remove",
            "clipboard-pin",
            "clipboard-qrcode",
            "clipboard-clear",
            "clipboard-sync",
            "clipboard-down",
            "clipboard-up",
            "clipboard-more",
            "clipboard-less",
            // NETWORK and SYNCTHING — the seventh view and the service the distro
            // installs. `network-connect` and `network-forget` act on the focused
            // row and that is why they only have keys INSIDE the view; the others
            // have keys in the tree.
            "network-open",
            "network-airplane",
            "theme-open",
            "world-glance",
            "locale-open",
            "locale-window",
            "locale-less",
            "locale-more",
            "locale-zone",
            "locale-place",
            "locale-where",
            "theme-apply",
            "network-scan",
            "network-connect",
            "network-disconnect",
            "network-forget",
            "network-window",
            "network-up",
            "network-down",
            "network-lan",
            "network-import",
            "network-section-next",
            "atuin-open",
            "atuin-window",
            "atuin-sync",
            "atuin-down",
            "atuin-up",
            "atuin-enter",
            "atuin-more",
            "atuin-less",
            "atuin-back",
            "network-tor-open",
            "network-tor-more",
            "network-tor-less",
            "network-tor-back",
            "network-section-previous",
            "network-wifi-toggle",
            "network-link",
            "syncthing-open",
            "syncthing-window",
            "syncthing-web",
            "syncthing-state",
            "syncthing-pause-all",
            "syncthing-resume-all",
            "syncthing-up",
            "syncthing-down",
            "syncthing-less",
            "syncthing-more",
            "syncthing-enter",
            "syncthing-pause",
            "syncthing-rescan",
            "syncthing-add",
            "syncthing-forget",
            "syncthing-add-device",
            "syncthing-add-folder",
            "window-menu",
            "drawer-open",
            "trail-open",
            "selection-open",
            // THE CHROME KIOSK — every one an arm of `run_with`, which hands
            // it to `kiosk::Kiosks::command` or to the kiosk's view.
            "kiosk-open",
            "kiosk-back",
            "kiosk-forward",
            "kiosk-reload",
            "kiosk-tab",
            "kiosk-tab-next",
            "kiosk-tab-previous",
            "kiosk-tab-close",
            "kiosk-tab-left",
            "kiosk-tab-right",
            "kiosk-tab-move",
            "kiosk-new-tab",
            "kiosk-address",
            "kiosk-zoom",
            "kiosk-zoom-in",
            "kiosk-zoom-out",
            "kiosk-zoom-reset",
            "kiosk-chrome-ui",
            "kiosk-address-line",
            "kiosk-more",
            "kiosk-tabs",
            "kiosk-extensions",
            "kiosk-extension",
            "kiosk-extension-options",
            "kiosk-extension-details",
            "kiosk-release",
            "modeline-adapt-kiosk",
            "kiosk-list-down",
            "kiosk-list-up",
            "kiosk-list-enter",
            "kiosk-tab-earlier",
            "kiosk-tab-later",
            "footer-hide-push",
            "footer-hide-cover",
            // THE STRIP'S THREE ABOUT A NOTICE, and the actions a module
            // offers: each an arm of `run_with`, which asks the brain.
            "brain-dismiss",
            "brain-snooze",
            "brain-act",
            // ── end of brain-ui ──
            "leader-set-default",
            "footer-bars-cycle",
            "leader-close",
            "minibuffer-fold-open",
            "minibuffer-fold-close",
            "alert-dismiss",
            "footer-settings-open",
            "footer-settings-window",
            "footer-settings-up",
            "footer-settings-down",
            "footer-settings-enter",
            "footer-hours",
            "icons-open",
            "icons-window",
            "icons-nerd-fonts",
            "icons-lucide",
            "icons-unicode",
            "icons-colour",
            "calc-open",
            "calc-window",
            "player-rewind",
            "player-forward",
            "player-picture-fold",
            "player-save-queue-again",
            "player-countries-order",
            "player-lyrics-translate",
            "player-website",
            "player-listing-earlier",
            "player-listing-later",
            "player-listing-first",
            "player-listing-last",
            "player-listing-full",
            "player-listing-layout",
            "player-listing-source",
            "player-listing-letter",
            "player-listing-order",
            "video-metrics-open",
            "video-metrics-window",
            "video-metrics-scan",
            "video-metrics-up",
            "video-metrics-down",
            "video-metrics-less",
            "video-metrics-more",
            "video-metrics-enter",
            "video-metrics-page-back",
            "video-metrics-page-on",
            "video-metrics-layout",
            "video-metrics-search",
            "video-metrics-player",
            "video-metrics-kiosk",
            "video-metrics-download",
            "video-metrics-hide",
            "video-metrics-delete",
            "video-metrics-add-channel",
            "video-metrics-channel-score",
            "video-metrics-points",
            "video-metrics-days",
            "video-metrics-band",
            "video-metrics-words",
            "video-metrics-second-tags",
            "video-metrics-weight",
            "security-open",
            "security-window",
            "security-relax",
            "security-shields",
            "security-firewall",
            "security-check-password",
            "security-check-email",
            "security-check-file",
            "security-down",
            "security-up",
            "security-enter",
            "security-left",
            "security-right",
            "security-rule-add",
            "security-rule-delete",
            "security-enrol",
            "security-rule",
            "security-vt-key",
            "security-vt-forget",
            "security-home",
            "containers-open",
            "containers-window",
            "containers-down",
            "containers-up",
            "containers-enter",
            "containers-left",
            "containers-right",
            "containers-next-kind",
            "containers-previous-kind",
            "containers-next-tab",
            "containers-previous-tab",
            "containers-start-stop",
            "containers-restart",
            "containers-remove",
            "containers-logs",
            "containers-shell",
            "containers-inspect",
            "containers-pull",
            "containers-build",
            "containers-login",
            "containers-login-password",
            "containers-prune",
            "containers-reveal",
            // THE BELONGINGS (`LEADER s v`, `ronoctua_shell_belongings::belongings`).
            "belongings-open",
            "belongings-window",
            "belongings-sync",
            "belongings-up",
            "belongings-down",
            "belongings-less",
            "belongings-more",
            "belongings-enter",
            "belongings-keep-local",
            "belongings-take-remote",
            "belongings-keep-both",
            "belongings-apply",
            "belongings-abandon",
            "belongings-all-local",
            "belongings-all-remote",
            "belongings-repository",
            "belongings-add",
            "belongings-auth",
            "belongings-keygen",
            "belongings-copy-key",
            "belongings-token",
            "belongings-user",
            "belongings-passphrase",
            "belongings-period",
            "fast-access-open",
            "fast-access-window",
            "fast-access-column",
            "fast-access-float",
            "fast-access-launch",
            "fast-access-open-path",
            "fast-access-reveal",
            "fast-access-man",
            // THE RANKING and its keys, arms of `run_with`.
            "player-ranking",
            "player-ranking-more",
            "player-ranking-earlier",
            "player-ranking-later",
            "player-ranking-first",
            "player-ranking-last",
            "player-ranking-layout",
            "display-rise",
            "display-sink",
            "clipboard-window",
            // THE 2026-10-05 WAVE'S, each an arm of `run_with` that says
            // "not yet" until its package lands.
            "belongings-pin",
            "belongings-sign",
            "software-sign",
            "belongings-take-merged-ours",
            "belongings-take-merged-theirs",
            "private-open",
            "private-lock",
            "private-show",
            "private-passphrase",
            "private-new-passphrase",
            "private-repeat-passphrase",
            "private-cap",
            "dev-clone",
            "stick-open",
            "stick-window",
            "stick-down",
            "stick-up",
            "stick-enter",
            "stick-reread",
            "security-encrypt",
            "security-quantum",
            "incus-open",
            "incus-window",
            "incus-new",
            "incus-images",
            "incus-down",
            "incus-up",
            "incus-enter",
            "incus-left",
            "incus-right",
            "incus-next-kind",
            "incus-previous-kind",
            "incus-next-tab",
            "incus-previous-tab",
            "incus-launch",
            "incus-start-stop",
            "incus-restart",
            "incus-freeze",
            "incus-delete",
            "incus-console",
            "incus-screen",
            "incus-shell",
            "incus-push",
            "incus-pull",
            "incus-export",
            "incus-import",
            "incus-copy-image",
            "incus-name",
            "incus-image",
            "incus-cpu",
            "incus-memory",
            "incus-processes",
            "incus-snapshot",
            "incus-push-to",
            "incus-pull-to",
            "incus-publish",
            "agents-open",
            "agents-window",
            "agents-session",
            "agents-login",
            "agents-up",
            "agents-down",
            "agents-enter",
            "agents-rename",
            "agents-default",
            "agents-remove",
            "agents-refresh",
            "agents-add-claude",
            "agents-add-codex",
            "agents-status-line",
            "agents-warn-at",
            "agents-danger-at",
        ];
        let (tree, registry) = build().unwrap();
        for id in reachable(&tree.full) {
            // The ones that are COMPOSITOR deeds go through the table,
            // consulted by id — and the generator proves, against the
            // compositor's own source, that every one of them exists.
            if ronoctua_shell_kit::actions::deed_of(id).is_some() {
                continue;
            }
            // The compositor's DEEDS are dispatched by prefix: `run_with`
            // looks the id up in the generated table and sends the text.
            // Some of them DO have keys, because a chord names one.
            if id.starts_with("wm-") {
                continue;
            }
            // The two workspace ones that take a number have no null form:
            // the chord carries the number in the bind itself.
            if matches!(id, "workspace-go" | "workspace-move") {
                continue;
            }
            // THE UPDATE'S THREE are dispatched by the one question the
            // footer asks its window (`update_window::order_of`), the same
            // function `run_with` calls — so being answered there IS being
            // handled.
            assert!(
                WITH_ACTION.contains(&id)
                    || ronoctua_shell_update::update_window::order_of(id).is_some(),
                "`{id}` has a key and `surface::run_command` does not handle it: \
                 it would show up in the list and do nothing"
            );
        }
        // And the inverse: an id in the list above that left the registry would become a
        // dead branch in the `match`, waiting for a key that does not exist.
        for id in WITH_ACTION.iter().chain(
            ronoctua_shell_update::update_window::COMMANDS
                .iter()
                .map(|c| &c.id),
        ) {
            assert!(
                registry.search(id).is_some(),
                "`{id}` is in the handled list and no longer exists"
            );
        }
    }

    #[test]
    fn with_caps_lock_a_group_still_opens() {
        // THE CRITERION, and not the function. This is literally the screen the
        // owner sent: `CTRL ALT SPC I is not bound`. With the Lock on xkb delivers
        // the capital, that sequence is bound to nothing, and EVERY lowercase
        // binding of the keymap dies with it.
        //
        // The LETTER is only the example. It was the icons' `i` on the day of
        // the incident; the browsers now live under `m`, so `m` is the
        // first-level group this walks through.
        use ronoctua_keymap::{Key, Resolution, Sym};
        // THE DEFAULT SET, which is where a sequence starts — and `m` folds
        // in none.
        let (trees, _) = build().expect("the default keymap has to close");
        let tree = &trees.footer;

        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char('M'))]),
                Resolution::Nothing
            ),
            "if `M` becomes a shortcut, this test has lost its point"
        );

        let c = ronoctua_shell_door::door::keys::without_caps_lock('M', true);
        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char(c))]),
                Resolution::Prefix(_)
            ),
            "with Caps Lock on, `LEADER m` has to open the group"
        );
    }

    #[test]
    fn every_command_a_gesture_runs_is_registered() {
        let (_, registry) = build().expect("the registry");
        for e in [
            ronoctua_gestures::Event::Focus(ronoctua_gestures::Direction::Up),
            ronoctua_gestures::Event::Focus(ronoctua_gestures::Direction::Right),
            ronoctua_gestures::Event::Focus(ronoctua_gestures::Direction::Down),
            ronoctua_gestures::Event::Focus(ronoctua_gestures::Direction::Left),
            ronoctua_gestures::Event::Workspace(ronoctua_gestures::Direction::Up),
            ronoctua_gestures::Event::Workspace(ronoctua_gestures::Direction::Down),
            ronoctua_gestures::Event::Overview,
            ronoctua_gestures::Event::Keyboard,
            ronoctua_gestures::Event::Player,
            ronoctua_gestures::Event::Close,
        ] {
            let id = ronoctua_shell_footer::gestures::command_of(e).expect("a command");
            assert!(registry.search(id).is_some(), "{id} is not a command");
        }
    }

    /// FAILS IF: a sentence of the software view teaches a key that, typed
    /// where every sequence starts, does something else — the queue's was
    /// `t t`, which in the default set is the queue and then a key that
    /// leads nowhere — or one of the three comes out empty.
    #[test]
    fn the_taught_keys_are_the_default_sets() {
        let taught =
            ronoctua_shell_kit::bindings::with_keymap(build().expect("the default keymap"), || {
                ronoctua_shell_software::software_feed::Taught::of_this_leader()
            });
        let leader = label();
        assert_eq!(taught.queue, format!("{leader} t"));
        assert_eq!(taught.launcher, format!("{leader} {}", APPS_KEYS));
        assert_eq!(taught.brain, format!("{leader} {}", BRAIN_KEYS));
    }

    /// FAILS IF: a command has no owner or two — a crate binds, names as
    /// reached by name only, or reaches from its view a command nobody
    /// registers; a crate binds or names a command another crate registers
    /// (its keys would then live away from the view that answers them); two
    /// crates register one command, or go by one name; or a command the
    /// footer runs by name outside every table (an event's action) is not a
    /// command of the registry.
    ///
    /// Each crate's keys are its own (`bindings::Contribution`): the command,
    /// the sequences that run it, what its view's keys and rows reach. With
    /// the single table of before, every command was "the leader's", and
    /// nothing could ask whose a key was.
    #[test]
    fn every_command_has_an_owner() {
        // THE ASSEMBLY REFUSES a crate that binds, or names as reached by
        // name only, a command it does not register (`bindings::assemble`),
        // and two crates that register one command: that it builds is the
        // first half of this test.
        let (_, registry) = build().expect("the keymap builds");
        let all = super::CONTRIBUTIONS;
        let owners_of = |id: &str| -> Vec<&'static str> {
            all.iter()
                .filter(|c| {
                    c.commands.iter().any(|k| k.id == id)
                        || (c.generated)().commands.iter().any(|k| k.id == id)
                })
                .map(|c| c.owner)
                .collect()
        };
        // ONE OWNER FOR EVERY COMMAND OF THE REGISTRY.
        for c in registry.all_ones() {
            let owners = owners_of(c.id);
            assert_eq!(owners.len(), 1, "`{}` is registered by {owners:?}", c.id);
        }
        // EVERY OWNER NAMED, and no name twice.
        let mut names: Vec<&str> = all.iter().map(|c| c.owner).collect();
        names.sort_unstable();
        let counted = names.len();
        names.dedup();
        assert_eq!(names.len(), counted, "two contributions go by one name");
        assert!(!names.contains(&""), "a contribution with no name");
        // WHAT A VIEW'S KEYS AND ROWS RUN IS SOMEBODY'S.
        for c in all {
            for id in (c.reached)() {
                assert!(
                    registry.search(id).is_some(),
                    "`{}`'s view runs `{id}`, which nobody registers",
                    c.owner
                );
            }
        }
        // THE COMMAND AN INSTALL'S LINE OFFERS, and every command an entry of
        // the distro's book names (`events::actions_of` draws only what the
        // registry knows, and the events' own test sees the book whole).
        assert!(
            registry
                .search(ronoctua_shell_footer::events::LAUNCH_LATEST)
                .is_some()
        );
        for entry in &ronoctua_notify::Book::distro().entries {
            for action in &entry.actions {
                if let ronoctua_notify::Doing::Command(id) = &action.doing {
                    assert!(
                        registry.search(id).is_some(),
                        "the book's `{}` offers `{id}`, which nobody registers",
                        entry.id
                    );
                }
            }
        }
    }
}
