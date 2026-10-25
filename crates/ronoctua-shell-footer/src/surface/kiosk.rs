//! ONE OF THE FOOTER'S OWN VIEWS: the Chrome kiosk (`crate::kiosk`, `crate::kiosk_view`) — the footer's own view.
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's
//! dispatchers reach it by one line each.

use super::*;

impl Footer {
    /// The kiosk a command acts on: the one the open view is about, else the
    /// focused window when it is one, else the one the `,` group was built
    /// for.
    pub(super) fn kiosk_target(&self) -> Option<u64> {
        self.kiosk_view
            .as_ref()
            .map(|v| v.window)
            .or_else(|| {
                self.windows
                    .focused()
                    .map(|w| w.id)
                    .filter(|w| self.kiosk.pair_of(*w).is_some())
            })
            .or(self.local_runs_here)
    }

    /// What the kiosk's grid switches are switched to.
    pub(super) fn kiosk_facts(&self, window: u64) -> crate::kiosk_view::Facts {
        crate::kiosk_view::Facts {
            bars_shown: self.kiosk.pair_of(window).is_some() && !self.kiosk.hidden(window),
            address_line: self.modeline.address_line,
        }
    }

    /// A kiosk command — the one door the registry, the view, the modeline
    /// and the `,` group share. It never runs a command back through
    /// `run_with`: what it cannot do itself, `kiosk::Kiosks::command` answers.
    pub(super) fn kiosk_run(&mut self, id: &str, argument: Option<&str>) {
        use crate::kiosk_view::Level;
        // THE ROW UNDER A LIST'S CURSOR is the argument a list's command takes
        // by name — `LEADER :` and the `,` group act on what the list shows.
        let from_the_view = argument.is_none().then(|| {
            self.kiosk_view
                .as_ref()
                .and_then(|v| v.argument_for(id, self.kiosk.reading(v.window)))
        });
        let arg: Option<String> = argument.map(ToOwned::to_owned).or(from_the_view.flatten());
        match id {
            "kiosk-more" => self.open_kiosk_view(Level::Root, None, true),
            "kiosk-tabs" => {
                let tab = arg.as_deref().and_then(|a| a.trim().parse().ok());
                self.open_kiosk_view(Level::Tabs, tab, false);
            }
            "kiosk-extensions" => self.open_kiosk_view(Level::Extensions, None, false),
            "kiosk-zoom" if arg.is_none() => self.open_kiosk_view(Level::Zoom, None, false),
            "kiosk-address-line" => self.flip_the_address_line(),
            "modeline-adapt-kiosk" => self.flip_the_adaptation(),
            _ => {
                let on = self.kiosk_target();
                let errands = self.kiosk.command(id, arg.as_deref(), on);
                self.kiosk_errands(errands);
                self.draw_and_paint();
            }
        }
    }

    /// Opens the kiosk's view at `level` — the tabs' cursor on `tab` — or,
    /// when `toggles` and it is open, closes it: the `…` is a switch, as the
    /// bell is. Already open, it goes to the level asked for.
    pub(super) fn open_kiosk_view(
        &mut self,
        level: crate::kiosk_view::Level,
        tab: Option<i64>,
        toggles: bool,
    ) {
        if let Some(v) = self.kiosk_view.as_mut() {
            if toggles {
                self.kiosk_view = None;
            } else {
                v.go(level, tab);
            }
            self.first_page();
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
            return;
        }
        let Some(window) = self.kiosk_target() else {
            let m = self.texts.t(crate::key::ECHO_KIOSK_NOT_A_KIOSK);
            self.echo_message(m);
            self.draw_and_paint();
            return;
        };
        // FROM THE WINDOW MENU'S `B`: this view is one of that menu's doors,
        // and takes the region from it rather than being refused by it.
        self.window_menu = None;
        if !self.region_is_free("kiosk-more") {
            return;
        }
        // UNDER THE HAND when a click on a line of the region opened it (the
        // window menu's `B`); from the modeline's `…` it grows above the line.
        let mut v =
            crate::kiosk_view::View::fresh(window, level).opened_under_the_hand(self.clicking);
        v.go(level, tab);
        self.kiosk_view = Some(v);
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// The compositor said something the kiosks care about.
    pub(super) fn kiosk_follows(&mut self, e: &ronoctua_compositor::Event) {
        let errands = self.kiosk.compositor(e, self.windows.all_windows());
        self.kiosk_errands(errands);
    }

    /// The bridge heard something.
    pub(super) fn kiosk_heard(&mut self, heard: ronoctua_browser::serve::Heard) {
        let errands = self.kiosk.heard(heard);
        self.kiosk_errands(errands);
        self.draw_and_paint();
    }

    /// Carries out what the kiosks asked for.
    ///
    /// AND SETTLES THE HEIGHT against what was APPLIED, never against a
    /// reading taken here: every caller has already moved the state — the
    /// bridge's state taken in, the focus moved onto or off a kiosk, the bars
    /// flipped — so "before" read here is already "after", and the address
    /// row would come and go inside a surface of the old height (the trap
    /// `display_file_changed` records).
    pub(super) fn kiosk_errands(&mut self, errands: Vec<crate::kiosk::Errand>) {
        use crate::kiosk::Errand;
        for errand in errands {
            match errand {
                Errand::Send(conn, envelope) => match self.kiosk_server.as_ref() {
                    Some(server) => {
                        if let Err(e) = server.send(conn, &envelope) {
                            tracing::warn!(conn, error = %e, "an order the bridge could not carry");
                        }
                    }
                    None => tracing::warn!("no bridge socket to send an order on"),
                },
                Errand::Bars { window, shown } => self.kiosk_bars(window, shown),
                Errand::Focus(window) => self.compositor_action(|wm| wm.focus_window(window)),
                // THE QUESTION IS ASKED ON THE MINIBUFFER'S LINE, starting
                // from what it is now — the address — and comes back as the
                // same command's argument.
                Errand::Ask { id, initial } => {
                    self.the_line_moves_on();
                    self.mx.request_with(id, false, initial.as_deref());
                    self.apply_geometry(KeyboardInteractivity::Exclusive);
                }
                Errand::Echo(said) => {
                    let m = self.texts.t(said);
                    self.echo_message(m);
                }
                Errand::Refused(why) => {
                    let m = self
                        .texts
                        .t_with(crate::key::ECHO_KIOSK_REFUSED, &[("why", &why)]);
                    self.echo_message(m);
                }
                Errand::Launch(launching) => self.kiosk_launch(launching),
                Errand::Persist => self.kiosk_persist(),
                Errand::ArmTimer(after) => self.kiosk_arm_timer(after),
            }
        }
        // A VIEW ABOUT A KIOSK THAT IS NOT ONE ANY MORE — released, its window
        // closed — goes with it.
        if self
            .kiosk_view
            .as_ref()
            .is_some_and(|v| self.kiosk.pair_of(v.window).is_none())
        {
            self.kiosk_view = None;
            self.first_page();
            self.apply_geometry(self.interactivity());
        } else if self.address_line_shown() != self.address_row_applied {
            self.apply_geometry(self.interactivity());
        }
    }

    /// Chrome's bars on a kiosk, shown or hidden — THE ONE PLACE the
    /// mechanism is named: the compositor's `fullscreen application` on that
    /// window, said ON or OFF and never the toggle, because the footer keeps
    /// the only copy of which it is.
    pub(super) fn kiosk_bars(&mut self, window: u64, shown: bool) {
        let id = if shown {
            "wm-full-application-off"
        } else {
            "wm-full-application-on"
        };
        let Some(spoken) = ronoctua_scroll::deeds::spoken_of(id) else {
            tracing::warn!(deed = id, "a deed the generated table does not have");
            return;
        };
        match open_the_compositor().and_then(|mut wm| wm.deed_on(window, &spoken.deed, None)) {
            Ok(()) => tracing::info!(window, shown, "the kiosk's bars"),
            Err(e) => {
                let m = format!("{e:#}");
                tracing::warn!(deed = id, window, error = %m, "the compositor refused");
                self.echo_message(m);
            }
        }
    }

    /// `kiosk.json` written again — the pairs a footer that restarts keeps.
    /// Not written is SAID, as every file the footer keeps for the person is
    /// (`say_a_preference_not_written`): the pairs hold in this process and
    /// are lost at its restart.
    pub(super) fn kiosk_persist(&mut self) {
        let Some(path) = ronoctua_xdg::runtime_file(KIOSK_FILE) else {
            return;
        };
        let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str()))
        else {
            return;
        };
        if let Err(e) = ronoctua_xdg::write_atomically(dir, file, &self.kiosk.kept()) {
            self.say_a_preference_not_written(KIOSK_FILE, &e);
        }
    }

    /// The clock that lets go of an unanswered launch — one at a time, and
    /// only while a launch waits: an event arms it, it never ticks at rest.
    pub(super) fn kiosk_arm_timer(&mut self, after: std::time::Duration) {
        if self.kiosk_timer {
            return;
        }
        let Some(handle) = self.repeat.clone() else {
            return;
        };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(after),
            |_, (), state: &mut Self| {
                state.kiosk_expired();
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(_) => self.kiosk_timer = true,
            Err(e) => tracing::warn!(error = %e, "the kiosk's clock could not be armed"),
        }
    }

    /// The clock rang.
    pub(super) fn kiosk_expired(&mut self) {
        self.kiosk_timer = false;
        let errands = self.kiosk.expired(std::time::Instant::now());
        self.kiosk_errands(errands);
        self.draw_and_paint();
    }

    /// Starts a kiosk window, on a thread of its own: the profile is read
    /// from Chrome's `Local State` — a file, and Chrome's — and the launch
    /// holds its thread for as long as a Chrome it started runs. The verdict
    /// comes back with the nonce (`kiosk_launched`).
    pub(super) fn kiosk_launch(&mut self, launching: crate::kiosk::Launching) {
        let Some(sender) = self.kiosk_launch_sender.clone() else {
            tracing::error!("no launch channel; the kiosk was not opened");
            return;
        };
        let roots = self.software_context.as_ref().map(|c| c.roots.clone());
        let spawned = std::thread::Builder::new()
            .name("kiosk-launch".into())
            .spawn(move || {
                // THE PROFILE CHROME USED LAST, and `Default` when that is
                // not a profile directory this system opens a kiosk in.
                let profile = roots
                    .as_ref()
                    .and_then(|r| {
                        ronoctua_software::web::chrome_profiles(r)
                            .into_iter()
                            .next()
                    })
                    .map(|p| p.directory)
                    .filter(|d| ronoctua_browser::launch::is_profile_dir(d))
                    .unwrap_or_else(|| ronoctua_software::web::DEFAULT_PROFILE.to_owned());
                let argv = ronoctua_browser::launch::argv(&ronoctua_browser::launch::Launch {
                    chrome: &launching.chrome,
                    profile: &profile,
                    extension_id: &launching.extension,
                    nonce: &launching.nonce,
                    url: launching.url.as_deref(),
                });
                let mut words = argv.iter().map(|a| a.to_string_lossy().into_owned());
                let program = words.next().unwrap_or_default();
                let args: Vec<String> = words.collect();
                tracing::info!(profile, "a kiosk window is asked of Chrome");
                let verdict = crate::foreign::launch_now(&program, &args, None);
                let _ = sender.send((launching.nonce, verdict));
            });
        if let Err(e) = spawned {
            tracing::error!(error = %e, "the kiosk's launching thread could not start");
        }
    }

    /// A kiosk's launch verdict. A failure is said the way every launch's is
    /// (`launched`), and the kiosks stop waiting for it: nothing will answer,
    /// and "it opened as an ordinary window" twenty seconds later would be
    /// false.
    pub(super) fn kiosk_launched(&mut self, nonce: &str, verdict: crate::foreign::Launch) {
        if matches!(verdict, crate::foreign::Launch::Failed { .. }) {
            self.kiosk.launch_failed(nonce);
        }
        self.launched(verdict);
    }

    /// `kiosk-open [address or words]` — `LEADER m b`, and the launcher's
    /// `br.ronoctua.kiosk` entry with the address it was handed.
    pub(super) fn kiosk_open(&mut self, argument: Option<&str>) {
        // THE BRIDGE IS LOOKED FOR AGAIN: a rebuild may have installed it
        // since the footer started.
        let ours = kiosk_bridge();
        self.kiosk.bridge_found(ours.clone());
        // CHROME IS A PART, which may not have arrived: asked on every open.
        let chrome = self
            .software_context
            .as_ref()
            .and_then(|c| c.tools.chrome.clone())
            .filter(|c| c.exists());
        let nonce = ronoctua_browser::launch::nonce()
            .map_err(|e| tracing::warn!(error = %e, "no nonce for a kiosk"))
            .ok();
        let errands = self.kiosk.open(
            argument,
            crate::kiosk::Opening {
                chrome: chrome.as_deref(),
                extension: ours.as_deref(),
                nonce: nonce.as_deref(),
            },
            std::time::Instant::now(),
        );
        self.kiosk_errands(errands);
        self.draw_and_paint();
    }

    /// A key, with the kiosk's view open.
    pub(super) fn kiosk_view_key(&mut self, key: ronoctua_keymap::Key) {
        use ronoctua_keymap::Sym;
        let Some(mut v) = self.kiosk_view.clone() else {
            return;
        };
        let before = v.level;
        // THE NOTATION WITH SHIFT LEFT IN THE LETTER (`M`, `R`, `K` are cells
        // of their own) and `Ctrl` kept — how the cells spell it.
        let plain = ronoctua_keymap::Key {
            mods: ronoctua_keymap::Mods {
                ctrl: key.mods.ctrl,
                ..ronoctua_keymap::Mods::default()
            },
            sym: key.sym,
        };
        let errand = if v.is_a_grid() {
            match key.sym {
                Sym::Esc | Sym::Backspace => v.back(),
                _ => v.pressed(&plain.notation()),
            }
        } else {
            let Some(press) = crate::kiosk_view::press_of_key(v.level, plain) else {
                return;
            };
            let r = self.kiosk.reading(v.window);
            let rows = v.rows(r, self.screen_columns(), &self.texts);
            v.press(press, r, &rows)
        };
        self.kiosk_view = Some(v);
        self.kiosk_view_errand(errand, before);
    }

    /// A list's key, by its command's name — `LEADER :`.
    pub(super) fn kiosk_view_press(&mut self, press: crate::kiosk_view::Press) {
        let Some(mut v) = self.kiosk_view.clone().filter(|v| !v.is_a_grid()) else {
            return;
        };
        let before = v.level;
        let r = self.kiosk.reading(v.window);
        let rows = v.rows(r, self.screen_columns(), &self.texts);
        let errand = v.press(press, r, &rows);
        self.kiosk_view = Some(v);
        self.kiosk_view_errand(errand, before);
    }

    /// A click on the view: a grid's cell runs, as a cell of the leader does
    /// — except under the hand that opened it, where the first click picks
    /// (`View::clicked_cell`); a list's row is selected, and the second click
    /// is its `RET` (invariant 19).
    pub(super) fn kiosk_view_clicked(&mut self, n: usize) {
        let Some(mut v) = self.kiosk_view.clone() else {
            return;
        };
        let before = v.level;
        let errand = if v.is_a_grid() {
            v.clicked_cell(n)
        } else {
            let r = self.kiosk.reading(v.window);
            let rows = v.rows(r, self.screen_columns(), &self.texts);
            if v.clicked(r, n, &rows) {
                v.press(crate::kiosk_view::Press::Enter, r, &rows)
            } else {
                crate::kiosk_view::Errand::Redraw
            }
        };
        self.kiosk_view = Some(v);
        self.kiosk_view_errand(errand, before);
    }

    /// What the kiosk's view asked for. The command runs FIRST and the view
    /// closes after it, the window menu's order.
    pub(super) fn kiosk_view_errand(
        &mut self,
        errand: crate::kiosk_view::Errand,
        before: crate::kiosk_view::Level,
    ) {
        use crate::kiosk_view::Errand;
        match errand {
            Errand::Redraw => {}
            Errand::Close => self.kiosk_view = None,
            Errand::Run { id, arg, stays } => {
                self.kiosk_run(id, arg.as_deref());
                if !stays {
                    self.kiosk_view = None;
                }
            }
        }
        // A NEW LEVEL, OR NO VIEW, starts on its first page.
        if self.kiosk_view.as_ref().is_none_or(|v| v.level != before) {
            self.first_page();
        }
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// The region's rows while it shows this view — the view's own line of
    /// `Footer::expansion`, the list the height and the drawing both read.
    pub(super) fn kiosk_expansion(&self) -> Vec<ExpansionRow> {
        let Some(v) = self.kiosk_view.as_ref() else {
            return Vec::new();
        };
        // A GRID IS THE WINDOW MENU'S CELLS — the key, the short label, the
        // registry's sentence for the pointer, and the `,` sequence that
        // does the same outside the view — and a list is rows of fields.
        if v.is_a_grid() {
            // AND THE CELL A CLICK PICKED, under the hand, in the cursor's
            // ink — the window menu's.
            let picked = v.picked();
            return v
                .entries(self.kiosk_facts(v.window), &self.texts)
                .into_iter()
                .enumerate()
                .map(|(n, e)| ExpansionRow {
                    key: e.key.to_owned(),
                    here: picked == Some(n),
                    key_color: if picked == Some(n) {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_KEY
                    },
                    text_color: if picked == Some(n) {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                    text: e.label,
                    teaches: Some(format!(
                        "{} {} {}",
                        crate::bindings::label(),
                        ronoctua_protocol::LOCAL_PREFIX,
                        e.key
                    )),
                    description: Some(e.about),
                    click: Some(Click::Kiosk(n)),
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        v.rows(
            self.kiosk.reading(v.window),
            self.screen_columns(),
            &self.texts,
        )
        .into_iter()
        .enumerate()
        .map(|(n, r)| ExpansionRow {
            click: (!r.heading).then_some(Click::Kiosk(n)),
            fields: r.fields,
            foot: false,
            ..ExpansionRow::empty_one()
        })
        .collect()
    }

    /// The row the cursor is on, which the region's page follows — the
    /// view's own line of `Footer::list_cursor`.
    pub(super) fn kiosk_cursor(&self) -> Option<usize> {
        // THE KIOSK'S LISTS, by the row the tab or the extension is drawn
        // on — the cursor keeps its id, not its row. A grid has none.
        self.kiosk_view.as_ref().and_then(|v| {
            let r = self.kiosk.reading(v.window);
            v.drawn_cursor(r, &v.rows(r, self.screen_columns(), &self.texts))
        })
    }

    /// The keys the minibuffer's line teaches while the view is shown —
    /// the view's own line of `Footer::view_hint`.
    pub(super) fn kiosk_hint(&self) -> Option<Vec<crate::chrome::Piece>> {
        // THE KIOSK'S VIEW: the grid names the tab it acts on, the
        // zoom says where it is, and a list teaches its keys.
        let texts = &self.texts;
        let kiosk_view = &self.kiosk_view;
        let kiosks = &self.kiosk;
        kiosk_view
            .as_ref()
            .map(|v| v.hint(kiosks.reading(v.window), texts))
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them. The view's own line of `Footer::run_with`.
    pub(super) fn kiosk_commands(&mut self, id: &str, argument: Option<&str>) -> bool {
        match id {
            // THE CHROME KIOSK (`LEADER m b`): the door, the view's list keys
            // by name, and every other kiosk command through `kiosk_run` —
            // the one door the view, the modeline and the `,` group share.
            "kiosk-open" => self.kiosk_open(argument),
            "kiosk-list-down" | "kiosk-list-up" | "kiosk-list-enter" | "kiosk-tab-earlier"
            | "kiosk-tab-later" => {
                if let Some(p) = crate::kiosk_view::press_of(id) {
                    self.kiosk_view_press(p);
                }
            }
            kiosk if kiosk.starts_with("kiosk-") || kiosk == "modeline-adapt-kiosk" => {
                self.kiosk_run(kiosk, argument);
            }
            _ => return false,
        }
        true
    }
}
