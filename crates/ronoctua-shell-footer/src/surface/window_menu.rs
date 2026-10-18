//! ONE OF THE FOOTER'S OWN VIEWS: the window menu and the drawer (`crate::window_menu`) — the footer's own view.
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's
//! dispatchers reach it by one line each.

use super::*;

impl Footer {
    /// Opens the window menu about the focused window — or closes it, when it
    /// is the menu the click came from.
    ///
    /// The window is ASKED for, and not taken from the kept state: sticking is
    /// a toggle the compositor sends no event about, and an item drawn on the
    /// wrong side of a toggle does the opposite of what it says.
    pub(super) fn open_window_menu(&mut self) {
        // CLOSED HERE, and not through `window_menu_errand`: that one runs
        // commands, and a command that opens this menu would close a cycle
        // among the footer's methods (`the-footer-does-not-ask-itself`).
        if self.window_menu.take().is_some() {
            self.first_page();
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
            return;
        }
        let asked = open_the_compositor()
            .and_then(|mut wm| wm.focused_window())
            .map_err(|e| tracing::warn!(error = %e, "the focused window could not be read"))
            .ok()
            .flatten()
            .or_else(|| self.windows.focused().cloned());
        let drawer = self.in_the_drawer();
        // NOTHING FOCUSED, BUT SOMETHING PUT AWAY: the drawer is the one part
        // of the menu that needs no window, and it is probably what was wanted
        // — putting the last window away leaves nothing focused.
        let Some(window) = asked else {
            if drawer.is_empty() {
                let m = self.texts.t(crate::key::WINDOW_MENU_NONE);
                self.echo_message(m);
            } else {
                self.open_drawer();
            }
            return;
        };
        if !self.region_is_free("window-menu") {
            return;
        }
        let screens = self.screens.iter().filter(|t| t.configured).count();
        // A KIOSK'S MENU HAS ITS VIEW ON `B` — the pointer's way to the
        // kiosk's toolbar when the modeline is not adapted to it.
        let kiosk = self.kiosk.pair_of(window.id).is_some();
        self.window_menu = Some(
            crate::window_menu::View::fresh(window, self.windows.workspaces().to_vec(), screens)
                .with_kiosk(kiosk)
                .with_drawer(drawer)
                .with_trails(self.windows.trails())
                .with_selection(self.selection.clone()),
        );
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// Opens the window menu straight at `level` — the trails' or the
    /// selection's mark, clicked. About the focused window when there is one
    /// (both levels have an item for "this window"), and alone when there is
    /// not.
    pub(super) fn open_window_menu_at(&mut self, level: crate::window_menu::Level) {
        if self.window_menu.is_none() {
            let focused = self.windows.focused().is_some();
            if focused {
                self.open_window_menu();
            } else if self.region_is_free("window-menu") {
                self.window_menu = Some(
                    crate::window_menu::View::alone(level)
                        .with_trails(self.windows.trails())
                        .with_selection(self.selection.clone()),
                );
            }
        }
        if let Some(v) = self.window_menu.as_mut() {
            v.level = level;
            self.first_page();
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            self.draw_and_paint();
        }
    }

    /// The windows of the drawer — put away in it, or let out and still its —
    /// as the compositor last listed them.
    ///
    /// Both, because a window taken out still BELONGS to the drawer: the next
    /// `put back` returns it there, and the mark going away the moment one
    /// came out read as the drawer having forgotten it.
    pub(super) fn in_the_drawer(&self) -> Vec<ronoctua_compositor::Window> {
        self.windows
            .all_windows()
            .iter()
            .filter(|w| w.drawer)
            .cloned()
            .collect()
    }

    /// Opens the window menu at the drawer, with no window to be about — the
    /// drawer's mark, clicked, or `LEADER w g`.
    pub(super) fn open_drawer(&mut self) {
        let drawer = self.in_the_drawer();
        if drawer.is_empty() {
            let m = self.texts.t(crate::key::WINDOW_MENU_DRAWER_EMPTY);
            self.echo_message(m);
            return;
        }
        if self.window_menu.is_some() || !self.region_is_free("drawer-open") {
            return;
        }
        self.window_menu = Some(crate::window_menu::View::of_the_drawer(drawer));
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// What the window menu asked for.
    ///
    /// # THE DEED FIRST, AND THE MENU CLOSES AFTER IT
    ///
    /// The other way round was a race in the compositor, measured in the VM:
    /// closing the menu shrinks the footer — the room windows have changes —
    /// and a `fullscreen` that arrived while that change was being taken in
    /// kept the column's OLD x, the screen's size hanging off its right edge
    /// until the next event. Fullscreen first and the footer shrinking after
    /// put it at 0, every time; so did the same two with no menu at all, in
    /// that order, and the failure came back in the other order with no menu
    /// either. Three guesses before this — the focus, an animation, a layout
    /// not sent — were wrong, and are not in the code.
    pub(super) fn window_menu_errand(&mut self, errand: crate::window_menu::Errand) {
        use crate::window_menu::Errand;
        if matches!(errand, Errand::Redraw) {
            // A NEW LEVEL starts on its first page.
            self.first_page();
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            self.draw_and_paint();
            return;
        }
        let stays = matches!(errand, Errand::Run { stays: true, .. });
        match errand {
            Errand::Redraw | Errand::Close => {}
            Errand::Run { id, arg, on, .. } => self.run_on(id, arg.as_deref(), on),
            Errand::TakeOut(id) => self.compositor_action(|wm| wm.take_out(id)),
            // THE PROCESS, by the pid the compositor gave for this window. A
            // `KILL` is the one signal a frozen program cannot refuse, which is
            // why a person reaches for it — and why it asked first.
            Errand::Kill(pid) => {
                let sent = rustix::process::Pid::from_raw(pid).is_some_and(|p| {
                    rustix::process::kill_process(p, rustix::process::Signal::KILL).is_ok()
                });
                if !sent {
                    tracing::warn!(pid, "the window's process could not be killed");
                    let m = self.texts.t(crate::key::ECHO_NO_WINDOW);
                    self.echo_message(m);
                }
            }
        }
        // AND THE WINDOW'S STATE NOW, before anything is painted: a deed that
        // put the window in fullscreen has it OVER the footer already, and
        // the footer painted while covered waits for a frame the compositor
        // does not send to a surface nobody sees — the owner's footer froze
        // for thirty-six seconds, the fullscreen event unread in its queue
        // until the window left fullscreen by a key. Asked here, the edge is
        // armed and the footer is on the overlay layer (committed without a
        // frame) before the paint below.
        self.fullscreen_asked_now();
        // A STEP LEAVES THE MENU UP — the next click is the next step.
        if !stays {
            self.window_menu = None;
            self.first_page();
            self.apply_geometry(self.interactivity());
        }
        self.draw_and_paint();
    }

    /// A key of the view, while it holds the keyboard — the view's own
    /// line of `Footer::view_took_the_key`.
    pub(super) fn window_menu_key(&mut self, key: ronoctua_keymap::Key) {
        // THE WINDOW MENU: every cell is a key, as in the leader, and `Esc`
        // goes up one level. The key is read as its NOTATION with shift left
        // in the letter (`K` is its own cell) and `Ctrl` kept (`C-h` nudges),
        // which is how the cells spell it.
        use ronoctua_keymap::Sym;
        let errand = match (key.sym, self.window_menu.as_mut()) {
            (_, None) => return,
            (Sym::Esc | Sym::Backspace, Some(v)) => v.back(),
            (sym, Some(v)) => {
                let notation = ronoctua_keymap::Key {
                    mods: ronoctua_keymap::Mods {
                        ctrl: key.mods.ctrl,
                        ..ronoctua_keymap::Mods::default()
                    },
                    sym,
                }
                .notation();
                v.pressed(&notation, &self.texts)
            }
        };
        self.window_menu_errand(errand);
    }

    /// The region's rows while it shows this view — the view's own line of
    /// `Footer::expansion`, the list the height and the drawing both read.
    pub(super) fn window_menu_expansion(&self) -> Vec<ExpansionRow> {
        let Some(v) = self.window_menu.as_ref() else {
            return Vec::new();
        };
        // THE LEADER'S CELL: the key, the short label, and the registry's
        // whole sentence for the pointer resting on it. A group wears the
        // `+` and the group's tone, as the leader's do.
        v.entries(&self.texts)
            .into_iter()
            .enumerate()
            .map(|(n, e)| {
                let group = e.is_group();
                ExpansionRow {
                    key: e.key.to_owned(),
                    text: if group {
                        format!("{GROUP_MARK}{}", e.label)
                    } else {
                        e.label
                    },
                    teaches: crate::window_menu::taught(self.tree, &e.does),
                    description: e.about,
                    click: Some(Click::WindowMenu(n)),
                    text_color: if group {
                        tokens::color::TEXT_GROUP
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                    ..ExpansionRow::empty_one()
                }
            })
            .collect()
    }

    /// A click on a line of the view — the view's own line of
    /// `Footer::click_at`.
    pub(super) fn window_menu_click(&mut self, n: usize) {
        // ONE CLICK RUNS IT, as a cell of the leader does: the grid has
        // no cursor to select with, and what a cell does is written on
        // it. What cannot be undone opens a question instead.
        let errand = match self.window_menu.as_mut() {
            Some(v) => v.choose(n, &self.texts),
            None => return,
        };
        self.window_menu_errand(errand);
    }

    /// The keys the minibuffer's line teaches while the view is shown —
    /// the view's own line of `Footer::view_hint`.
    pub(super) fn window_menu_hint(&self) -> Option<Vec<crate::chrome::Piece>> {
        // THE WINDOW MENU names the window it is about: its cells are
        // their own keys, so the line has nothing else to teach, and
        // the way back goes in front of the name.
        let window_menu = &self.window_menu;
        window_menu.as_ref().map(|v| {
            v.window
                .as_ref()
                .and_then(|w| w.title.clone().or_else(|| w.app_id.clone()))
                .map(crate::chrome::Piece::prose)
                .into_iter()
                .collect()
        })
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them. The view's own line of `Footer::run_with`.
    pub(super) fn window_menu_commands(&mut self, id: &str, _argument: Option<&str>) -> bool {
        match id {
            "window-menu" => self.open_window_menu(),
            "drawer-open" => self.open_drawer(),
            "trail-open" => self.open_window_menu_at(crate::window_menu::Level::Trail),
            "selection-open" => self.open_window_menu_at(crate::window_menu::Level::Selection),
            _ => return false,
        }
        true
    }
}
