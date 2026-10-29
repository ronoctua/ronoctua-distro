//! ONE OF THE FOOTER'S OWN VIEWS: the theme chooser (`crate::theme_chooser`) — the footer's own view.
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's
//! dispatchers reach it by one line each.

use super::*;

impl Footer {
    /// Writes a previewed or chosen pair everywhere, and tells the compositor
    /// to look NOW.
    ///
    /// # Why the compositor is told rather than left to notice
    ///
    /// Because it never notices. The desk is an `output * bg` in a fragment
    /// the compositor `include`s, and this compositor watches no file of its
    /// own — there is no inotify anywhere in it. Writing the file and stopping
    /// there would leave the footer changing colour and the desk staying the
    /// colour it was, for the rest of the session.
    ///
    /// The previous compositor DID watch, and the defect was subtler: the desk
    /// arrived "sometimes a little faster, sometimes a little slower", which is
    /// what the owner read off the screen, because a watcher has a cadence.
    /// Asking is the same one unix-socket round trip either way.
    pub(super) fn show_theme_everywhere(&mut self, choice: crate::theme::Choice) {
        // A PREVIEW THE FILE REFUSED is said, every time: the footer wears it
        // and the other doors do not, and the line is where that difference
        // is explained (`theme::NotWritten::Kept`).
        if let Err(e) = crate::theme::show_everywhere(choice) {
            let text = e.said(&self.texts);
            self.echo_message(text);
        }
        self.tell_the_compositor_to_reread(choice);
    }

    /// The theme chooser's preview of `choice`, whichever gesture moved a
    /// cursor — an arrow or a click: this door wears it, marks it as its OWN
    /// write (see `theme_changed`: the watcher hands it back a moment later,
    /// and it must not be read as somebody's choice), and shows it to every
    /// other door through the file.
    pub(super) fn preview_theme(&mut self, choice: crate::theme::Choice) {
        crate::theme::wear(choice);
        if let Some(v) = self.theme_chooser.as_mut() {
            v.previewing(choice);
        }
        self.show_theme_everywhere(choice);
    }

    /// A key of the view, while it holds the keyboard — the view's own
    /// line of `Footer::view_took_the_key`.
    pub(super) fn themes_key(&mut self, key: ronoctua_keymap::Key) {
        // THE THEME CHOOSER: the network list's keys, with one difference —
        // moving a cursor is a PREVIEW, so the choice is shown on the way and
        // put back on `Esc`. See `theme_chooser.rs` for why a preview writes
        // the file every other door watches.
        use ronoctua_keymap::Sym;
        match key.sym {
            Sym::Esc => {
                if let Some(v) = self.theme_chooser.take() {
                    // EVERYTHING back: this door's own colours in memory,
                    // and — through the file every other door watches —
                    // the windows, the compositor's desk and the fragments
                    // new terminals read. The file goes back to what it
                    // said, or AWAY when there was none: a machine that
                    // never chose must not be given a file by having
                    // looked at the list.
                    crate::theme::wear(v.committed());
                    // AND A FILE THAT IS NOT WHAT THE CHOOSER LEFT — one
                    // that no longer reads whole — is left alone, and
                    // the line says why.
                    if let Err(e) = crate::theme::put_everything_back(v.committed(), v.before()) {
                        let text = e.said(&self.texts);
                        self.echo_message(text);
                    }
                    // AND THE DESK COMES BACK IN THE SAME BREATH, for the
                    // reason it went: the compositor never notices on its
                    // own, and `Esc` would leave the previewed desk — and the
                    // previewed spacing — on screen for the rest of the
                    // session.
                    self.tell_the_compositor_to_reread(v.committed());
                }
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            Sym::Down | Sym::Up => {
                let step = if key.sym == Sym::Down { 1 } else { -1 };
                // EVERY CURSOR PREVIEWS THE WHOLE CHOICE — the five
                // answers, whichever column the keyboard is in — and a
                // cursor at its end previews nothing new, so nothing is
                // written. Taken inside the borrow and shown outside it:
                // `v` holds `self`, and telling the compositor is the
                // footer's own business.
                let shown = self.theme_chooser.as_mut().and_then(|v| v.move_by(step));
                if let Some(c) = shown {
                    self.preview_theme(c);
                }
                self.draw_and_paint();
            }
            Sym::Left | Sym::Right => {
                // FROM COLUMN TO COLUMN, five of them, each keeping its
                // own cursor: crossing previews nothing, because no
                // answer changed.
                let step = if key.sym == Sym::Right { 1 } else { -1 };
                if let Some(v) = self.theme_chooser.as_mut() {
                    v.cross(step);
                }
                self.draw_and_paint();
            }
            otherwise => {
                let notation = ronoctua_keymap::Key::simple(otherwise).notation();
                if let Some((_, id)) = crate::leader::OF_THEME_VIEW
                    .iter()
                    .find(|(t, _)| *t == notation)
                {
                    self.run_command(id);
                }
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
        }
    }

    /// The region's rows while it shows this view — the view's own line of
    /// `Footer::expansion`, the list the height and the drawing both read.
    pub(super) fn themes_expansion(&self) -> Vec<ExpansionRow> {
        // THE THEME CHOOSER BEFORE THE MANAGER, as `view()` ranks them. The
        // two are never open at once (`theme-open` refuses while the manager
        // is up, because the region's height, grid and pointer maths are the
        // manager's), so the order here is `view()`'s order and nothing more.
        let Some(v) = self.theme_chooser.as_ref() else {
            return Vec::new();
        };
        // FIVE LISTS SIDE BY SIDE — the themes and the four choices about
        // a theme — each laid out by the chooser as FIELDS of its own
        // (`theme_chooser::View::lines`), the shape the notification
        // manager's two columns already have. They were two lists poured
        // into one key/text pair, and the pair is one row to everything
        // downstream: the pointer's band ran across both lists, which is
        // what the owner reported. As fields, every column has its own
        // cells, and the band, the click and the rules all ask the one
        // `theme_chooser::Layout` where they are.
        //
        // The colours of the rows are the theme under the cursors',
        // because that choice IS the process's while the cursors rest
        // there — the list is its own swatch.
        let columns = self.screen_columns();
        v.lines(&self.texts, columns)
            .into_iter()
            .enumerate()
            .map(|(i, fields)| ExpansionRow {
                // THE TITLES' ROW TAKES NO CLICK, so the pointer lights
                // nothing there: a title is not a place for a cursor.
                click: (i > 0).then_some(Click::Theme(i)),
                fields,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// A click on a line of the view — the view's own line of
    /// `Footer::click_at`.
    pub(super) fn themes_click(&mut self, n: usize) {
        // WHICH LIST comes from the pointer's PIXEL, through the same
        // `theme_chooser::Layout` the band and the rules are drawn
        // from — so the column that lights is the column the click
        // takes, to the pixel of the rule between them. Past the last
        // column there is none, and nothing happens.
        #[allow(clippy::cast_possible_truncation)]
        let Some(column) = self.here.x_under_pointer.and_then(|x| {
            crate::theme_chooser::View::column_under(
                self.screen_columns(),
                x as f32,
                crate::chrome::cell_width(),
            )
        }) else {
            return;
        };
        let Some(clicked) = self.theme_chooser.as_mut().map(|v| v.clicked(column, n)) else {
            return;
        };
        match clicked {
            // THE POINTER PREVIEWS WHAT THE ARROWS DO — the whole
            // choice, wherever it landed.
            crate::theme_chooser::Clicked::Previewed(c) => {
                self.preview_theme(c);
                self.draw_and_paint();
            }
            crate::theme_chooser::Clicked::Focused => self.draw_and_paint(),
            // THE SECOND CLICK IS `RET` (invariant 19): the command
            // the view's table binds to it, run the way the key runs
            // it.
            crate::theme_chooser::Clicked::Again => {
                self.run_command("theme-apply");
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            crate::theme_chooser::Clicked::Nothing => {}
        }
    }

    /// The row the cursor is on, which the region's page follows — the
    /// view's own line of `Footer::list_cursor`.
    pub(super) fn themes_cursor(&self) -> Option<usize> {
        self.theme_chooser
            .as_ref()
            .map(crate::theme_chooser::View::row)
    }

    /// The keys the minibuffer's line teaches while the view is shown —
    /// the view's own line of `Footer::view_hint`.
    pub(super) fn themes_hint(&self) -> Option<Vec<crate::chrome::Piece>> {
        // THE THEME CHOOSER: `RET` applies, `Esc` goes back, and
        // nothing on its rows says so.
        let theme_chooser = &self.theme_chooser;
        let texts = &self.texts;
        theme_chooser.as_ref().map(|v| v.hint(texts))
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them. The view's own line of `Footer::run_with`.
    pub(super) fn themes_commands(&mut self, id: &str, _argument: Option<&str>) -> bool {
        match id {
            // ---------------------------------------------------------
            // NETWORK. The view opens with what NetworkManager ALREADY
            // KNOWS: the list of access points lives in the daemon's
            // memory, and giving it back is one trip to the bus. What
            // takes time is the radio's scan, and that one is NOT waited
            // for — see `network.rs`.
            // ---------------------------------------------------------
            // THE THEME CHOOSER. Opening is a list; applying writes the file,
            // which is what reaches every other door (`theme.rs`), and closes
            // the list on the theme that is now on disk. A refusal to write
            // keeps the list open and says why on the echo line: a choice that
            // looked applied and was not would come back at the next start.
            "theme-open" => {
                // NOT OVER THE FOOTER'S MANAGER. Only the socket door can ask for
                // it then (the leader cannot be typed while the manager grabs),
                // and the region's height, grid and pointer maths are the
                // manager's while it is open: two rows on top of the region
                // with the strip painted under them. The manager holds
                // state a person built by hand; a cursor over a few rows can
                // wait until it closes.
                if !self.region_is_free("theme-open") {
                    return true;
                }
                // Opened twice — the socket door does not know the list is up —
                // it keeps the live view: a fresh one would read the PREVIEW as
                // the committed theme, and `Esc` would then leave it on screen.
                if self.theme_chooser.is_none() {
                    self.theme_chooser = Some(crate::theme_chooser::View::fresh_one(
                        crate::theme::committed(),
                        // ASKED ONCE, BEFORE THE FIRST PREVIEW. From here on
                        // the file exists whether or not anybody chose, because
                        // that is how a preview reaches the other doors — and
                        // a file that did not read whole is one no preview
                        // may write over (`theme::Before`).
                        crate::theme::before(),
                    ));
                }
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            "theme-apply" => {
                let Some(v) = self.theme_chooser.as_mut() else {
                    // No key runs it outside the list; a name typed into
                    // `M-x` with the list closed has nothing to apply.
                    return true;
                };
                let chosen = v.focused_choice();
                match crate::theme::persist(chosen) {
                    Ok(()) => {
                        v.commit();
                        crate::theme::wear(chosen);
                        self.theme_chooser = None;
                        self.first_page();
                        self.apply_geometry(self.interactivity());
                    }
                    // A FILE THAT DOES NOT READ WHOLE IS NOT WRITTEN OVER,
                    // and the line says so — with what did not read — as it
                    // says a disk that refused.
                    Err(e) => {
                        let text = e.said(&self.texts);
                        self.echo_message(text);
                    }
                }
            }
            _ => return false,
        }
        true
    }
}
