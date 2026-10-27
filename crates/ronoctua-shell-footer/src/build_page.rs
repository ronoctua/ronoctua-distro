//! `LEADER / p`, and the build glyph's right-click — the build switch's page:
//! what the feature is, which build is running, and the one setting that says
//! when the glyph is drawn.
//!
//! # What is here
//!
//! - A HEADING and a note: the machine's OWN build (`/etc/nixos`, its
//!   `host.nix`) against a distro checkout's REFERENCE build, which comes up on
//!   the distro's defaults — a different layout, profile and hostname.
//! - Three READ-ONLY rows ([`crate::chrome::Kind::Fact`], which the cursor
//!   skips): the build running, its hostname, and the checkout it would switch
//!   to.
//! - The GLYPH SETTING ([`crate::build_switch::Show`]): `Dev` (the default — a
//!   mark only on a reference build), `Always` or `Never`, walked by `RET`,
//!   written to `modeline.json` the one way both of the footer's doors write it
//!   (`Footer::write_the_modeline`).
//! - THE SWITCH itself, when there is a checkout: `RET` on it runs
//!   `build-toggle`, the same command `LEADER / b` and the glyph's left-click
//!   run.
//!
//! # Where the answers live
//!
//! The setting is in `modeline.json` (`crate::adapt::Prefs`), beside the bars
//! and the kiosk, so the footer already watches it; the build and the checkout
//! are read from the generation (`crate::build_switch::State`). The view holds
//! only the cursor and draws everything from what the footer hands it, as
//! `crate::footer_settings` does.

use crate::build_switch::{Build, Show, State};
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::key;
use ronoctua_tokens as tokens;

/// The keys this view answers to. The hint line is GENERATED from it
/// (`chrome::hint_of`), so a key that moves here moves on screen.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "build-page-down"),
    ("<up>", "build-page-up"),
    ("RET", "build-page-enter"),
];

/// The command that walks the glyph setting — the footer runs it, writing
/// `modeline.json` (`Footer::cycle_build_show`), as a row of
/// `crate::footer_settings` runs its switch's command.
pub const SHOW_CYCLE_COMMAND: &str = "build-show-cycle";

/// One row that ACTS — the cursor stops on it and `RET` does something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Walk the glyph setting to its next value.
    Show,
    /// Switch to the other build — `build-toggle`.
    Switch,
}

impl Act {
    /// The command `RET` on its row runs.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Show => SHOW_CYCLE_COMMAND,
            Self::Switch => "build-toggle",
        }
    }
}

/// Every command this view's keys and rows reach — for `bindings::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id)| *id)
        .chain([Act::Show.command(), Act::Switch.command()])
}

/// One drawn row.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// The act it runs — only on an acting row's first line.
    pub act: Option<Act>,
    /// A title, a sentence, a fact or the rest of a label: the cursor skips it.
    pub heading: bool,
}

impl Row {
    /// Whether a click takes it — an acting row, which the cursor stands on;
    /// so also whether a resting pointer lights it. Titles, facts, notes and
    /// the air take nothing.
    #[must_use]
    pub const fn takes(&self) -> bool {
        !self.heading
    }
}

/// The view: where the cursor is. Everything it draws is read from the state
/// and the preferences the footer hands it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    cursor: usize,
}

impl View {
    /// Opens it, on the first row that acts.
    #[must_use]
    pub fn fresh(state: &State, show: Show, columns: u16, texts: &ronoctua_i18n::Texts) -> Self {
        let mut me = Self::default();
        let rows = me.rows(state, show, columns, texts);
        me.settle_on(&rows);
        me
    }

    /// Where the cursor is, for the door that draws a band.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Puts the cursor where a click landed — on a row that acts — and answers
    /// whether it was ALREADY there (the second click, which the door turns
    /// into `RET`, invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(Row::takes) {
            let again = self.cursor == n;
            self.cursor = n;
            return again;
        }
        false
    }

    /// The sentence the pointer resting on row `n` reads — the command's own,
    /// which is what `RET` there does.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let act = rows.get(n)?.act?;
        Some(texts.t(&format!("cmd-{}", act.command())))
    }

    /// The rows to draw, at this width, from the state and the setting as they
    /// are.
    #[must_use]
    pub fn rows(
        &self,
        state: &State,
        show: Show,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let end = left + room;
        let mut out: Vec<Row> = Vec::new();

        // THE TITLE, and the note that says what the two builds are.
        out.push(heading(left, texts.t(key::BUILD_PAGE_TITLE), room));
        for line in crate::chrome::note_lines(&texts.t(key::BUILD_PAGE_NOTE), room) {
            out.push(note(left, line, room));
        }

        // THE FACTS AND THE SETTING SHARE ONE COLUMN, after the widest label.
        let build_value = texts.t(match state.build {
            Build::Own => key::BUILD_PAGE_BUILD_OWN,
            Build::Reference => key::BUILD_PAGE_BUILD_REFERENCE,
        });
        let host_value = state
            .hostname
            .clone()
            .unwrap_or_else(|| texts.t(key::BUILD_PAGE_UNKNOWN));
        let checkout_value = state.checkout.as_ref().map_or_else(
            || texts.t(key::BUILD_PAGE_NO_CHECKOUT),
            |p| p.display().to_string(),
        );
        let show_value = texts.t(match show {
            Show::Dev => key::BUILD_PAGE_SHOW_DEV,
            Show::Always => key::BUILD_PAGE_SHOW_ALWAYS,
            Show::Never => key::BUILD_PAGE_SHOW_NEVER,
        });
        let facts = [
            (texts.t(key::BUILD_PAGE_BUILD), build_value),
            (texts.t(key::BUILD_PAGE_HOST), host_value),
            (texts.t(key::BUILD_PAGE_CHECKOUT), checkout_value),
        ];
        let show_label = texts.t(key::BUILD_PAGE_SHOW);
        let width = ronoctua_render::layout::width;
        let pairs: Vec<(u16, u16, u16)> = facts
            .iter()
            .map(|(l, v)| (left, width(l), width(v)))
            .chain(std::iter::once((
                left,
                width(&show_label),
                width(&show_value),
            )))
            .collect();
        let column = crate::chrome::section_column(left, room, &pairs);

        // THE SECTION'S TITLE, then the facts, each a row the cursor skips.
        out.push(air());
        out.push(heading(left, texts.t(key::BUILD_PAGE_RUNNING), room));
        for (label, value) in &facts {
            for fields in crate::chrome::aligned_pair(
                left,
                column,
                end,
                label,
                value,
                crate::chrome::Kind::Fact,
                false,
            ) {
                out.push(Row {
                    fields,
                    act: None,
                    heading: true,
                });
            }
        }

        // THE GLYPH SETTING, which acts.
        out.push(air());
        out.push(heading(left, texts.t(key::BUILD_PAGE_GLYPH), room));
        let focused = self.cursor == out.len();
        let lines = crate::chrome::aligned_pair(
            left,
            column,
            end,
            &show_label,
            &show_value,
            crate::chrome::Kind::Setting,
            focused,
        );
        push_acting(&mut out, lines, Act::Show);
        for line in crate::chrome::note_lines(&texts.t(key::BUILD_PAGE_SHOW_NOTE), room) {
            out.push(note(left, line, room));
        }

        // THE SWITCH, when there is a checkout to switch to — its label says
        // which build it lands on.
        if state.checkout.is_some() {
            out.push(air());
            let switch = texts.t(match state.other_build() {
                Build::Own => key::BUILD_PAGE_SWITCH_TO_OWN,
                Build::Reference => key::BUILD_PAGE_SWITCH_TO_REFERENCE,
            });
            let focused = self.cursor == out.len();
            let lines = crate::chrome::aligned_pair(
                left,
                column,
                end,
                &switch,
                "",
                crate::chrome::Kind::Setting,
                focused,
            );
            push_acting(&mut out, lines, Act::Switch);
        }
        out
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows that were DRAWN.
    pub fn press(
        &mut self,
        p: Press,
        state: &State,
        show: Show,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(state, show, columns, texts);
        self.settle_on(&rows);
        match p {
            Press::Up => {
                self.walk(&rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(&rows, true);
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Enter => rows
                .get(self.cursor)
                .and_then(|r| r.act)
                .map_or(Errand::Nothing, Errand::Act),
        }
    }

    /// Moves the cursor to the next row that acts, round the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(Row::takes) {
                self.cursor = at;
                return;
            }
        }
    }

    /// Puts the cursor on a row that acts — never on a title or a fact.
    pub fn settle_on(&mut self, rows: &[Row]) {
        if rows.get(self.cursor).is_some_and(Row::takes) {
            return;
        }
        self.cursor = rows.iter().position(Row::takes).unwrap_or(0);
    }
}

/// A key, reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one acting row.
    Up,
    /// Down one acting row.
    Down,
    /// Act on the row under the cursor.
    Enter,
    /// Close.
    Escape,
}

/// A key, by its WHOLE notation, as the view's press. `Esc` closes whatever is
/// held.
#[must_use]
pub fn press_of_key(k: ronoctua_keymap::Key) -> Option<Press> {
    if k.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = k.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    press_of(id)
}

/// A command of [`OF_THE_VIEW`] as the view's press — a click on the hint, or
/// the id run by name.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "build-page-down" => Press::Down,
        "build-page-up" => Press::Up,
        "build-page-enter" => Press::Enter,
        _ => return None,
    })
}

/// What the view wants done after a key or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Run this act's command — the footer walks the setting or switches.
    Act(Act),
}

/// One acting row's lines pushed whole: the act is on the first line, the rest
/// are headings the cursor skips.
fn push_acting(out: &mut Vec<Row>, lines: Vec<Vec<Field>>, act: Act) {
    for (n, fields) in lines.into_iter().enumerate() {
        out.push(Row {
            fields,
            act: (n == 0).then_some(act),
            heading: n > 0,
        });
    }
}

/// A title line, in the heading's colour.
fn heading(left: u16, text: String, room: u16) -> Row {
    Row {
        fields: vec![piece(left, text, tokens::color::TEXT_HEADING, room)],
        act: None,
        heading: true,
    }
}

/// A sentence line, dim.
fn note(left: u16, text: String, room: u16) -> Row {
    Row {
        fields: vec![piece(left, text, tokens::color::TEXT_DIM, room)],
        act: None,
        heading: true,
    }
}

/// A blank line.
fn air() -> Row {
    Row {
        fields: crate::chrome::AIR,
        act: None,
        heading: true,
    }
}

/// One piece of text, elided to `room`.
fn piece(at: u16, text: String, color: tokens::Color, room: u16) -> Field {
    Field {
        at,
        text: ronoctua_render::layout::elide_end(&text, room),
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn state(build: Build, checkout: Option<&str>) -> State {
        State {
            build,
            hostname: Some("noctua-z16".to_owned()),
            checkout: checkout.map(PathBuf::from),
        }
    }

    /// FAILS IF: the cursor is born on a title, the setting and the switch are
    /// not both reachable, or a reference build with no checkout still offers
    /// the switch.
    #[test]
    fn the_cursor_reaches_the_setting_and_the_switch() {
        let t = texts("en-GB");
        let s = state(Build::Reference, Some("/home/p/distro"));
        let v = View::fresh(&s, Show::Dev, 80, &t);
        let rows = v.rows(&s, Show::Dev, 80, &t);
        assert!(rows[v.cursor()].takes(), "the cursor was born on a title");
        let acts: Vec<Act> = rows.iter().filter_map(|r| r.act).collect();
        assert_eq!(acts, [Act::Show, Act::Switch]);
        // NO CHECKOUT: the setting is still there, the switch is not.
        let bare = state(Build::Own, None);
        let acts: Vec<Act> = View::default()
            .rows(&bare, Show::Dev, 80, &t)
            .iter()
            .filter_map(|r| r.act)
            .collect();
        assert_eq!(acts, [Act::Show]);
    }

    /// FAILS IF: `RET` on the setting runs anything but the cycle command, on
    /// the switch anything but `build-toggle`, the arrows stop on a fact, or
    /// `Esc` does not close.
    #[test]
    fn the_keys_walk_the_rows_and_run_their_commands() {
        let t = texts("en-GB");
        let s = state(Build::Reference, Some("/home/p/distro"));
        let mut v = View::fresh(&s, Show::Dev, 80, &t);
        assert_eq!(
            v.press(Press::Enter, &s, Show::Dev, 80, &t),
            Errand::Act(Act::Show)
        );
        v.press(Press::Down, &s, Show::Dev, 80, &t);
        assert_eq!(
            v.press(Press::Enter, &s, Show::Dev, 80, &t),
            Errand::Act(Act::Switch)
        );
        assert_eq!(Act::Show.command(), SHOW_CYCLE_COMMAND);
        assert_eq!(Act::Switch.command(), "build-toggle");
        assert_eq!(v.press(Press::Escape, &s, Show::Dev, 80, &t), Errand::Close);
    }

    /// FAILS IF: a value, label or note of the page has no words in a
    /// language — a missing key draws as `[key]`.
    #[test]
    fn the_page_has_words_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for build in [Build::Own, Build::Reference] {
                for show in [Show::Dev, Show::Always, Show::Never] {
                    let s = state(build, Some("/home/p/distro"));
                    let rows = View::default().rows(&s, show, 100, &t);
                    let drawn: String = rows
                        .iter()
                        .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    assert!(!drawn.contains('['), "{language}: a missing key: {drawn}");
                }
            }
        }
    }
}
