//! ONE OF THE FOOTER'S OWN VIEWS: the app finder (`crate::apps`) — the footer's own view, and its icons.
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's
//! dispatchers reach it by one line each.

use super::*;

impl Footer {
    /// Orders a re-sweep if some `applications` directory changed.
    ///
    /// # Why it is still not a sweep per key
    ///
    /// Because what runs here is one `stat` per directory — three or four calls
    /// —, and the sweep itself, when it does need to happen, goes to the thread.
    /// The list opens with the catalogue that already exists and is SWAPPED when
    /// the new one arrives; nothing waits. Without this, an application installed
    /// after startup would stay invisible until the next session — and "I
    /// installed it and it does not show up" is the defect that makes a person
    /// doubt the installer.
    pub(super) fn rescan_if_changed(&mut self) {
        let now = crate::launch::stamp();
        if now == self.apps_stamp {
            return;
        }
        let Some(sender) = self.catalogue_sender.clone() else {
            return;
        };
        tracing::info!("the application directories changed; rescanning");
        self.apps_stamp = now;
        crate::launch::scan(self.texts.language(), sender);
    }

    /// Is this `Icon=`'s pixmap already decoded?
    ///
    /// It gives back the KEY, and not the pixels: whoever draws needs it to ask
    /// the GPU whether the texture is already up, and walking around with a
    /// reference to the pixels would hold the borrow until the end of the painting.
    pub(super) fn pixmap_ready(&self, icon: &str) -> Option<String> {
        // THE KEY CARRIES THE SIDE, and it is the same key the GPU is asked
        // about: `crate::launch::pixmap_key` says why at length. Built here from
        // the LIVE side, so a change of text size asks a question the cache has
        // not been asked before, instead of finding yesterday's answer.
        let key = crate::launch::pixmap_key(icon, self.icon_side_px());
        match self.icons.get(&key) {
            Some(Some(_)) => Some(key),
            _ => None,
        }
    }

    /// Asks the thread for the pixmaps of the VISIBLE lines that are still missing.
    ///
    /// # Why only the visible ones
    ///
    /// Because decoding four hundred SVGs costs seconds, and three hundred and
    /// eighty of them are not on screen. What is seen is at most the region's
    /// ceiling in lines, and scrolling asks for the next such page. The cache
    /// keeps what has already arrived, so scrolling back repeats nothing.
    pub(super) fn request_visible_icons(&mut self) {
        let Some(queue) = self.icon_queue.clone() else {
            return;
        };
        if self.apps.is_none() {
            return;
        }
        let (offset, per_page) = (self.current_offset(), self.grid().per_page());
        let Some(l) = self.apps.as_ref() else {
            return;
        };
        let side = self.icon_side_px();
        // ASKED FOR BY THE SAME KEY IT WILL BE FILED UNDER, side included:
        // asking by the bare name would find the picture of another size
        // already there and never request this one.
        let missing_ones: Vec<String> = (offset..offset + per_page)
            .filter_map(|i| l.at(i))
            .map(|e| e.icon.clone())
            .filter(|n| {
                let key = crate::launch::pixmap_key(n, side);
                !n.is_empty()
                    && !self.icons.contains_key(&key)
                    && !self.icons_requested.contains(&key)
            })
            .collect();
        for icon in missing_ones {
            // A closed queue means the thread died. Registering the request
            // anyway would leave the name stuck in "requested" forever, and
            // the line would never try again.
            if queue
                .send(crate::launch::Request {
                    icon: icon.clone(),
                    side,
                })
                .is_err()
            {
                self.icon_queue = None;
                return;
            }
            self.icons_requested
                .insert(crate::launch::pixmap_key(&icon, side));
        }
    }

    /// The new catalogue, coming from the sweeping thread.
    pub(super) fn catalogued(&mut self, fresh: crate::launch::Catalogue) {
        self.catalogue = fresh;
        // AND EVERY OPEN REGION OF THE SLOT HOST hears it — the file
        // manager's "open with".
        let known = self.catalogue.clone();
        self.hosted_heard::<crate::launch::Catalogue>(&known);
        // WITH THE LIST OPEN, it updates itself WITHOUT losing what was typed.
        // Closing and reopening would wipe the query because of an event the
        // person did not cause.
        if let Some(l) = self.apps.as_mut() {
            l.recatalogue(self.catalogue.clone());
            self.follow_cursor();
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            self.draw_and_paint();
        }
    }

    /// A key with the app finder open.
    pub(super) fn apps_key(&mut self, key: ronoctua_keymap::Key) {
        let Some(l) = self.apps.as_mut() else {
            return;
        };
        let action = l.key(key);
        self.apply_app_action(action);
    }

    /// What the app finder asked for, with the screen already set right.
    pub(super) fn apply_app_action(&mut self, action: crate::apps::Action) {
        use crate::apps::Action;
        match action {
            Action::Nothing => {}
            // THE LIVE SELECTION, read off the loop and typed into the query
            // when it comes back (`PasteFor::Applications`).
            Action::WantsPaste => self.read_the_clipboard(PasteFor::Applications),
            // REDRAW HERE, and not just re-commit the size: with the list at the
            // ceiling, filtering from 400 down to 40 changes no height at all, and
            // the compositor only sends a `configure` when the height CHANGES.
            // Without this, typing would not filter on screen — the same defect the `M-x` had.
            Action::Changed => {
                self.follow_cursor();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            Action::Closed => {
                self.apps = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            // PINNED, AND THE LAUNCHER STAYS OPEN. Pinning is not opening.
            //
            // The preferences are read and written HERE rather than through a
            // `files::View` that may not exist: `LEADER a` is reachable without
            // the manager ever having been opened, and refusing to pin until it
            // has been would be a rule with nothing on screen to state it. The
            // manager notices on its own — it re-reads the file when its mtime
            // moves, and so does the window, which is another process entirely.
            Action::Pin(e) => {
                use ronoctua_files::sidebar::{Item, Target, store};
                // THE DIRECTORY IS FOUND HERE, once, and handed in — the
                // crate has no function that finds it on its own any more.
                // See `sidebar::store` for the defect that removed them.
                let Some(dir) = store::config_dir() else {
                    return;
                };
                let mut prefs = store::read_from(&dir);
                let id = format!("app:{}", e.id);
                if prefs.added.iter().any(|i| i.id == id) {
                    let m = self.texts.t_with(
                        crate::key::FILES_SIDEBAR_PINNED,
                        &[("name", e.name.as_str())],
                    );
                    self.echo_message(m);
                    self.draw_and_paint();
                    return;
                }
                prefs.added.push(Item {
                    id,
                    // THE ID, and never the command: a store path names a build
                    // that the next collection removes, and an id survives every
                    // rebuild. The NAME is stored because it is the one label
                    // that cannot be derived from the target.
                    target: Target::Application(e.id.clone()),
                    label: Some(e.name.clone()),
                });
                let m = match store::write_to(&dir, &prefs) {
                    Ok(()) => self.texts.t_with(
                        crate::key::FILES_SIDEBAR_PINNED,
                        &[("name", e.name.as_str())],
                    ),
                    Err(err) => self.texts.t_with(
                        crate::key::FILES_SIDEBAR_UNWRITABLE,
                        &[("why", err.to_string().as_str())],
                    ),
                };
                self.echo_message(m);
                self.draw_and_paint();
            }
            // THE SOFTWARE VIEW'S THREE DOORS. Each closes the launcher and
            // opens the view — the results for what was typed, or the sheet of
            // the program at `self.catalogue[i]` (the index is the catalogue's,
            // never a row's), with the remove question already up and on NO
            // for `Remove`. The launcher closes FIRST: the region holds one
            // view, and the software view asks for it.
            Action::SearchToInstall(query) => {
                self.apps = None;
                self.first_page();
                let words = format!("{} {query}", crate::programs_words::SEARCH_WORD);
                self.run_with("software-open", Some(&words));
            }
            Action::Details(i) => {
                self.apps = None;
                self.first_page();
                self.open_software_for(i, false);
            }
            Action::Remove(i) => {
                self.apps = None;
                self.first_page();
                self.open_software_for(i, true);
            }
            Action::Open(e) => {
                // CLOSE BEFORE LAUNCHING. The launch is asynchronous (see
                // `foreign::start_in_scope`), and leaving the list open on top
                // of the application that has just opened would be the footer
                // stealing the keyboard from the new window.
                self.apps = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.open_application(&e);
                self.draw_and_paint();
            }
        }
    }

    /// Brings up an application from the catalogue.
    ///
    /// # `Terminal=true` goes through OUR terminal
    ///
    /// Seven of 108 measured entries ask for a terminal, and they have no
    /// interface of their own: `htop` launched directly opens and dies with
    /// nothing on screen. They go by the same path as `buffer-terminal` —
    /// `current_exe terminal <prog>` —, and not by an `xterm -e` that would
    /// require one more emulator in the system.
    pub(super) fn open_application(&mut self, e: &ronoctua_apps::DesktopEntry) {
        // THE ONE ANSWER TO "HOW IS THIS ENTRY STARTED", which a window's
        // launch by desktop id asks too (`launch::launch_entry`).
        let Some((program, args, cwd)) = crate::launch::argv_of(e) else {
            tracing::error!("I do not know my own binary");
            return;
        };
        tracing::info!(app = %e.id, program, terminal = e.terminal, "opening the application");
        self.open_foreign(&program, &args, cwd.as_deref());
    }

    /// The launcher's `C-i` and `C-d`: the software view on the sheet of the
    /// program behind `self.catalogue[i]`, with the remove question up for
    /// `C-d` — asked of the view by its own command, with the entry's desktop
    /// id and name (`software_region::SHEET`): which program it is is the
    /// view's reading to say.
    pub(super) fn open_software_for(&mut self, i: usize, remove: bool) {
        let Some(e) = self.catalogue.get(i).cloned() else {
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
            return;
        };
        let command = if remove {
            crate::programs_words::SHEET_TO_REMOVE
        } else {
            crate::programs_words::SHEET
        };
        let argument = crate::programs_words::sheet_argument(&e.id, &e.name);
        self.run_with(command, Some(&argument));
    }

    /// Opens a launcher entry by its desktop id, through the launcher's own
    /// path — what a view's "start it" asks (`Launching::Entry`). One the
    /// catalogue does not have yet — installed a moment ago, before the sweep
    /// ran — asks for the sweep and says so.
    pub(super) fn launch_desktop(&mut self, desktop_id: &str) {
        let entry = self
            .catalogue
            .iter()
            .find(|e| crate::launch::same_desktop(&e.id, desktop_id))
            .cloned();
        match entry {
            Some(e) => self.open_application(&e),
            None => {
                self.rescan_if_changed();
                let said = self.texts.t(crate::key::SOFTWARE_NOT_IN_LAUNCHER);
                self.echo_message(said);
            }
        }
    }

    /// The region's rows while it shows this view — the view's own line of
    /// `Footer::expansion`, the list the height and the drawing both read.
    pub(super) fn apps_expansion(&self) -> Vec<ExpansionRow> {
        // THE APPLICATIONS, in the same region and by the same mechanism. Three
        // columns: the glyph, the name (with the search highlight) and the
        // description — which the drawing elides, because `Comment` is prose and
        // does not fit.
        let Some(l) = self.apps.as_ref() else {
            return Vec::new();
        };
        if l.total() == 0 && l.offers_search().is_none() {
            // THE SWEEP HAS NOT ARRIVED YET (or found nothing). Saying that
            // is better than an empty region, which is indistinguishable
            // from "the key did not work".
            return vec![ExpansionRow {
                text: self.texts.t(crate::key::APPS_EMPTY),
                text_color: tokens::color::TEXT_DIM,
                ..ExpansionRow::empty_one()
            }];
        }
        let query = l.normalized_query();
        l.result()
            .iter()
            .enumerate()
            .filter_map(|(i, _)| l.at(i).map(|e| (i, e)))
            .map(|(i, e)| ExpansionRow {
                picture: None,
                beside_picture: false,
                // THE PIXMAP ONCE IT HAS ARRIVED, the glyph while it has not.
                //
                // The choice is HERE, and in a single place: the two drawn
                // together would end up one over the other, and the decision
                // duplicated in the painting loop would diverge from the one
                // that decides the column's width.
                pixmap: self.pixmap_ready(&e.icon),
                pixmap_at: None,
                icon: self
                    .pixmap_ready(&e.icon)
                    .is_none()
                    .then(|| crate::launch::glyph_of(e)),
                // THE NAME in the left column, flush with it: it is what the
                // search matches and what the highlight lights up. Pushing four
                // hundred names right would give a jagged left margin.
                key: e.name.clone(),
                text: e.description.clone(),
                key_flush_right: false,
                // NO `:` BETWEEN THEM. `Firefox : navegador web` asserts a
                // relation that does not exist — the column already separates them.
                separator: false,
                aside: false,
                // THE HIGHLIGHT IS OVER THE NAME, and not over the folded key:
                // the folding is 1:1 in characters precisely so the positions
                // hold in both. See `ronoctua_apps::fold`.
                here: i == l.cursor(),
                highlights: crate::minibuffer::match_up(&query, &e.key)
                    .map(|(_, p)| p)
                    .unwrap_or_default(),
                click: Some(Click::Application(i)),
                // THE DESCRIPTION IS ALREADY ON THE LINE. Repeating it in the
                // message area on hover would show the same text twice; what
                // helps there is the command that is going to run.
                teaches: None,
                description: Some(e.command.0.clone()),
                key_color: if i == l.cursor() {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_PRIMARY
                },
                text_color: if i == l.cursor() {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_DIM
                },
                fields: Vec::new(),
                foot: false,
                chips: Vec::new(),
                caret: None,
                covers: Vec::new(),
                continues: false,
                lit: crate::region::LineLights::Row,
                chosen: None,
                region_row: None,
            })
            // THE LAST ROW: look for what was typed among what can be
            // INSTALLED. After the matches, and alone when there are none —
            // "not installed" is never a dead end. Its index is the one after
            // the last match, which is what `Finder::choose` takes.
            .chain(l.offers_search().map(|q| {
                let n = l.result().len();
                let here = l.is_search_row(l.cursor());
                ExpansionRow {
                    icon: Some(tokens::icon::SEARCH),
                    key: crate::apps::search_row_text(&self.texts, q),
                    key_flush_right: false,
                    separator: false,
                    here,
                    click: Some(Click::Application(n)),
                    description: Some(self.texts.t(crate::key::CMD_APPS_SEARCH_TO_INSTALL)),
                    key_color: if here {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_DIM
                    },
                    ..ExpansionRow::empty_one()
                }
            }))
            .collect()
    }

    /// A click on a line of the view — the view's own line of
    /// `Footer::click_at`.
    pub(super) fn apps_click(&mut self, n: usize) {
        let Some(l) = self.apps.as_mut() else {
            return;
        };
        // BY THE SAME `Enter` as the keyboard: nothing may be reachable only
        // by the mouse, nor behave differently depending on how one got
        // to it.
        let action = l.choose(n);
        self.apply_app_action(action);
        self.draw_and_paint();
    }

    /// The row the cursor is on, which the region's page follows — the
    /// view's own line of `Footer::list_cursor`.
    pub(super) fn apps_cursor(&self) -> Option<usize> {
        // This used to be a hand-written `match` because the function was
        // `const` and a `const fn` cannot call `Option::map`. It is not const
        // any more (see the note on the signature), so the longhand is just
        // longhand now.
        self.apps.as_ref().map(crate::apps::Finder::cursor)
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them. The view's own line of `Footer::run_with`.
    pub(super) fn apps_commands(&mut self, id: &str, _argument: Option<&str>) -> bool {
        match id {
            // THE LAUNCHER'S OWN KEY, and it only means anything with the
            // launcher open — which is where its key lives. Reached from `M-x`
            // with nothing focused it does nothing and says nothing, exactly
            // like `clipboard-pin` on an empty clipboard: the id exists so the
            // key is enumerable, and the action is where the state is.
            "apps-pin" | "apps-details" | "apps-remove" | crate::apps::SEARCH_TO_INSTALL => {
                // ONE door for the launcher's commands: `Finder::command` is
                // what its keys run too, so a key, a name and a click cannot
                // come to mean different things.
                let action = self.apps.as_ref().map(|l| l.command(id));
                if let Some(action) = action {
                    self.apply_app_action(action);
                }
            }
            "apps-open" => {
                if !self.region_is_free("apps-open") {
                    return true;
                }
                self.rescan_if_changed();
                tracing::info!(apps = self.catalogue.len(), "finder opened");
                self.apps = Some(crate::apps::Finder::fresh(self.catalogue.clone()));
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            _ => return false,
        }
        true
    }

    /// The clipboard's text the view asked for, read off the loop — the
    /// view's own line of `Footer::pasted`.
    pub(super) fn apps_pasted(&mut self, text: &str) {
        let action = self.apps.as_mut().map(|a| a.paste(text));
        if let Some(action) = action {
            self.apply_app_action(action);
        }
    }
}
