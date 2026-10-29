//! ONE OF THE FOOTER'S OWN VIEWS: the footer's own settings (`LEADER s r r`, `crate::footer_settings`) — the footer's own view.
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's
//! dispatchers reach it by one line each.

use super::*;

impl Footer {
    /// `footer-settings-open`: the footer's own settings, in the region —
    /// never over a view that holds work (`region_is_free`), and a second
    /// opening keeps the live view and its cursor.
    pub(super) fn open_footer_settings(&mut self) {
        if !self.region_is_free("footer-settings-open") {
            return;
        }
        if self.footer_settings.is_none() {
            self.footer_settings = Some(crate::footer_settings::View::fresh(
                &self.modeline,
                self.screen_columns(),
                &self.texts,
            ));
        }
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// A key of the footer's settings, from the keyboard, the hint or a
    /// second click — and what the view asks for after it.
    pub(super) fn footer_settings_press(&mut self, press: crate::footer_settings::Press) {
        use crate::footer_settings::Errand;
        let columns = self.screen_columns();
        let texts = std::rc::Rc::clone(&self.texts);
        let errand = self.footer_settings.as_mut().map_or(Errand::Nothing, |v| {
            v.press(press, &self.modeline, columns, &texts)
        });
        match errand {
            Errand::Nothing => self.draw_and_paint(),
            Errand::Close => {
                self.footer_settings = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            // THE ROW'S COMMAND, the one `LEADER :` and a tap reach too: it
            // writes the file (`Setting::change`), moves the bars, says a
            // refusal on this line, and ends in a frame.
            Errand::Change(setting) => self.run_command(setting.command()),
        }
    }

    /// A key of the view, while it holds the keyboard — the view's own
    /// line of `Footer::view_took_the_key`.
    pub(super) fn footer_settings_key(&mut self, key: ronoctua_keymap::Key) {
        // THE FOOTER'S OWN SETTINGS: two arrows and `RET`, read by the whole
        // notation through the reader the window calls too
        // (`footer_settings::press_of_key`) — `C-n` is not `<down>` in
        // either door.
        if let Some(press) = crate::footer_settings::press_of_key(key) {
            self.footer_settings_press(press);
        }
    }

    /// The region's rows while it shows this view — the view's own line of
    /// `Footer::expansion`, the list the height and the drawing both read.
    pub(super) fn footer_settings_expansion(&self) -> Vec<ExpansionRow> {
        // THE FOOTER'S OWN SETTINGS: the view's rows, each first line a
        // click that selects and a second that acts, and each one saying,
        // under the pointer, what its `RET` does.
        let Some(v) = self.footer_settings.as_ref() else {
            return Vec::new();
        };
        let rows = v.rows(&self.modeline, self.screen_columns(), &self.texts);
        rows.iter()
            .enumerate()
            .map(|(n, r)| ExpansionRow {
                // ONLY A ROW A CLICK TAKES (`Row::takes`, the window's answer
                // too) has a click — and so a band under the pointer.
                click: r.takes().then_some(Click::FooterSettings(n)),
                fields: r.fields.clone(),
                description: crate::footer_settings::View::about(&rows, n, &self.texts),
                foot: false,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// A click on a line of the view — the view's own line of
    /// `Footer::click_at`.
    pub(super) fn footer_settings_click(&mut self, n: usize) {
        // THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19,
        // through the same press a key goes through.
        let width = self.screen_columns();
        let again = self.footer_settings.as_mut().is_some_and(|v| {
            let rows = v.rows(&self.modeline, width, &self.texts);
            v.clicked(n, &rows)
        });
        if again {
            self.footer_settings_press(crate::footer_settings::Press::Enter);
        } else {
            self.draw_and_paint();
        }
    }

    /// The row the cursor is on, which the region's page follows — the
    /// view's own line of `Footer::list_cursor`.
    pub(super) fn footer_settings_cursor(&self) -> Option<usize> {
        self.footer_settings
            .as_ref()
            .map(crate::footer_settings::View::cursor)
    }

    /// The keys the minibuffer's line teaches while the view is shown —
    /// the view's own line of `Footer::view_hint`.
    pub(super) fn footer_settings_hint(&self) -> Option<Vec<crate::chrome::Piece>> {
        // THE FOOTER'S SETTINGS: its two arrows and `RET`, generated
        // from its table (`footer_settings::OF_THE_VIEW`).
        let texts = &self.texts;
        Some(crate::chrome::hint_of(
            crate::footer_settings::OF_THE_VIEW,
            texts,
        ))
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them. The view's own line of `Footer::run_with`.
    pub(super) fn footer_settings_commands(&mut self, id: &str, _argument: Option<&str>) -> bool {
        match id {
            "footer-settings-open" => self.open_footer_settings(),
            "footer-settings-window" => {
                if let Ok(me) = std::env::current_exe() {
                    self.open_foreign(&me.to_string_lossy(), &["footer-settings".to_owned()], None);
                }
            }
            "footer-settings-up" | "footer-settings-down" | "footer-settings-enter" => {
                if let Some(press) = crate::footer_settings::press_of(id) {
                    self.footer_settings_press(press);
                }
            }
            "footer-hours" => self.flip_the_hours(),
            "footer-hide-header" => self.flip_the_header(),
            "footer-hide-shortcuts" => self.flip_the_shortcuts(),
            _ => return false,
        }
        true
    }
}
