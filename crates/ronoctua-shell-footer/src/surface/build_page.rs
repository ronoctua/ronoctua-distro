//! ONE OF THE FOOTER'S OWN VIEWS: the build switch's page (`LEADER / p`, and
//! the glyph's right-click — `crate::build_page`).
//!
//! Not a region of a feature: it stays the footer's, ranked by name in
//! `PRECEDENCE` (`Rank`), and kept in its own file so the footer's dispatchers
//! reach it by one line each.

use super::*;

impl Footer {
    /// `build-open`: the build switch's page, in the region — never over a view
    /// that holds work (`region_is_free`), and a second opening keeps the live
    /// view and its cursor.
    pub(super) fn open_build_page(&mut self) {
        if !self.region_is_free("build-open") {
            return;
        }
        if self.build_page.is_none() {
            self.build_page = Some(crate::build_page::View::fresh(
                &self.build_state,
                self.modeline.build_show,
                self.screen_columns(),
                &self.texts,
            ));
        }
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// A key of the page — the view's own line of `Footer::view_took_the_key` —
    /// and what it asks for after it.
    pub(super) fn build_page_press(&mut self, press: crate::build_page::Press) {
        use crate::build_page::Errand;
        let columns = self.screen_columns();
        let texts = std::rc::Rc::clone(&self.texts);
        let show = self.modeline.build_show;
        let errand = self.build_page.as_mut().map_or(Errand::Nothing, |v| {
            v.press(press, &self.build_state, show, columns, &texts)
        });
        match errand {
            Errand::Nothing => self.draw_and_paint(),
            Errand::Close => {
                self.build_page = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            // THE ROW'S COMMAND, which `LEADER :` reaches too: the setting's
            // walks `modeline.json`, the switch's is `build-toggle`.
            Errand::Act(act) => self.run_command(act.command()),
        }
    }

    /// A key of the view, while it holds the keyboard.
    pub(super) fn build_page_key(&mut self, key: ronoctua_keymap::Key) {
        if let Some(press) = crate::build_page::press_of_key(key) {
            self.build_page_press(press);
        }
    }

    /// The region's rows while it shows this view — the list the height and the
    /// drawing both read.
    pub(super) fn build_page_expansion(&self) -> Vec<ExpansionRow> {
        let Some(v) = self.build_page.as_ref() else {
            return Vec::new();
        };
        let rows = v.rows(
            &self.build_state,
            self.modeline.build_show,
            self.screen_columns(),
            &self.texts,
        );
        rows.iter()
            .enumerate()
            .map(|(n, r)| ExpansionRow {
                // ONLY A ROW A CLICK TAKES has a click, and so a band under the
                // pointer.
                click: r.takes().then_some(Click::BuildPage(n)),
                fields: r.fields.clone(),
                description: crate::build_page::View::about(&rows, n, &self.texts),
                foot: false,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// A click on a line of the view — the click selects, and the second one
    /// acts (invariant 19), through the press a key goes through.
    pub(super) fn build_page_click(&mut self, n: usize) {
        let columns = self.screen_columns();
        let show = self.modeline.build_show;
        let again = self.build_page.as_mut().is_some_and(|v| {
            let rows = v.rows(&self.build_state, show, columns, &self.texts);
            v.clicked(n, &rows)
        });
        if again {
            self.build_page_press(crate::build_page::Press::Enter);
        } else {
            self.draw_and_paint();
        }
    }

    /// The row the cursor is on, which the region's page follows.
    pub(super) fn build_page_cursor(&self) -> Option<usize> {
        self.build_page
            .as_ref()
            .map(crate::build_page::View::cursor)
    }

    /// The keys the minibuffer's line teaches while the view is shown.
    pub(super) fn build_page_hint(&self) -> Option<Vec<crate::chrome::Piece>> {
        Some(crate::chrome::hint_of(
            crate::build_page::OF_THE_VIEW,
            &self.texts,
        ))
    }

    /// Walk the glyph setting one value, written to `modeline.json` the one way
    /// both of the footer's doors write it (`write_the_modeline`).
    fn cycle_build_show(&mut self) {
        self.write_the_modeline(|p| p.build_show = p.build_show.next());
    }

    /// The view's commands — the registry's, the leader's, `LEADER :`'s:
    /// `false` when `id` is none of them. The view's own line of
    /// `Footer::run_with`.
    pub(super) fn build_page_commands(&mut self, id: &str, _argument: Option<&str>) -> bool {
        match id {
            "build-open" => self.open_build_page(),
            "build-show-cycle" => self.cycle_build_show(),
            "build-page-up" | "build-page-down" | "build-page-enter" => {
                if let Some(press) = crate::build_page::press_of(id) {
                    self.build_page_press(press);
                }
            }
            _ => return false,
        }
        true
    }
}
