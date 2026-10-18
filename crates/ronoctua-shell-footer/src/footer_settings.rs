//! `LEADER s r` — the footer's own settings, in one view with two doors: the
//! footer's region (`s r r`) and a window (`s r w`,
//! `crate::footer_settings_window`).
//!
//! # What is here
//!
//! - THE BARS' MODE — shown, hidden and pushing, hidden and over the windows
//!   (`crate::hidden`). A tap of `Ctrl` at the leader's root walks the same
//!   three; this row is where a person SEES which one is on, since at rest
//!   the two hidden modes look alike.
//! - WHAT THE FREE MINIBUFFER SHOWS: one thing today, the hours, off unless
//!   asked for (the owner, 2026-09-30: the clock left invariant 14's list of
//!   what the chrome never has, and came back as a choice).
//! - THE MODELINE ADAPTING TO A KIOSK, which lived in the kiosk's own grid
//!   (`M`). It is a setting of the footer and not a deed on the tab in front,
//!   and the kiosk's grid is where a person acts on the page.
//!
//! # Where the answers live, and who writes them
//!
//! In `modeline.json` (`crate::adapt::Prefs`), the file the footer already
//! keeps its bars and its adapters in, merged by field. What `RET` on a row
//! does to the file is ONE arithmetic ([`Setting::change`]), laid over the
//! file as it is now, and each door writes it itself: the footer's runs the
//! row's command (the bars are the footer's, and it says their new mode), and
//! the window writes through the same `Store` ([`change_the_file`]). Each
//! says on ITS OWN line when the file will not parse or will not be written —
//! a refusal said in the other door, or only in a log, is one the person who
//! pressed the key never reads. The other door hears the change the way every
//! door hears a preference: the file (`Notice::on_file`), and the footer
//! moves its bars from it (`Footer::modeline_changed`).
//!
//! # One section, laid out whole
//!
//! Every row is a setting of one list, so the values begin in one column
//! after the widest label (`chrome::section_column`), and a label that does
//! not fit wraps under itself (`chrome::aligned_pair`) — the rule this wave
//! gives every new view.

use crate::key;
use ronoctua_tokens as tokens;

use crate::adapt::{Hidden, Prefs};
use crate::fields::{Field, Hit as FilesHit, Ink};

/// The keys this view answers to. The hint line is GENERATED from it
/// (`chrome::hint_of`), so a key that moves here moves on screen.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "footer-settings-down"),
    ("<up>", "footer-settings-up"),
    ("RET", "footer-settings-enter"),
];

/// The command that shows the hours on the free minibuffer, or stops.
pub const HOURS_COMMAND: &str = "footer-hours";

/// The command that adapts the modeline to a kiosk, or stops — the one the
/// kiosk's grid ran before it moved here.
pub const KIOSK_COMMAND: &str = "modeline-adapt-kiosk";

/// Every command this view's keys and rows reach — for `bindings::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id)| *id)
        .chain(Setting::ALL.iter().map(|s| s.command()))
}

/// One setting of the view — one row that acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// The bars' mode.
    Bars,
    /// The hours on the free minibuffer.
    Hours,
    /// The modeline adapting to a kiosk.
    Kiosk,
}

impl Setting {
    /// The three, in the order drawn.
    pub const ALL: [Self; 3] = [Self::Bars, Self::Hours, Self::Kiosk];

    /// The command `RET` on its row runs in the footer's door.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Bars => crate::hidden::CYCLE_COMMAND,
            Self::Hours => HOURS_COMMAND,
            Self::Kiosk => KIOSK_COMMAND,
        }
    }

    /// What `RET` on its row does to the file — the one arithmetic both doors
    /// write: the bars to their next mode (`crate::hidden::next_mode`), the
    /// hours and the kiosk's adapter the other way round. Laid over the file
    /// AS IT IS NOW, so a switch another hand made is the one turned.
    pub fn change(self, prefs: &mut Prefs) {
        match self {
            Self::Bars => prefs.hidden = crate::hidden::next_mode(prefs.hidden),
            Self::Hours => prefs.hours = !prefs.hours,
            Self::Kiosk => {
                let on = prefs.adapts(crate::kiosk::ADAPTER);
                prefs.adapt.insert(crate::kiosk::ADAPTER.to_owned(), !on);
            }
        }
    }

    /// Its section's title.
    const fn heading(self) -> &'static str {
        match self {
            Self::Bars => key::FOOTER_SETTINGS_BARS,
            Self::Hours => key::FOOTER_SETTINGS_FREE,
            Self::Kiosk => key::FOOTER_SETTINGS_KIOSK,
        }
    }

    /// What its row says.
    const fn label(self) -> &'static str {
        match self {
            Self::Bars => key::FOOTER_SETTINGS_BARS_MODE,
            Self::Hours => key::FOOTER_SETTINGS_HOURS,
            Self::Kiosk => key::FOOTER_SETTINGS_KIOSK_ADAPT,
        }
    }

    /// What its row's value says, as the file has it now.
    #[must_use]
    pub fn value(self, prefs: &Prefs) -> &'static str {
        let on = |yes: bool| {
            if yes {
                key::FOOTER_SETTINGS_ON
            } else {
                key::FOOTER_SETTINGS_OFF
            }
        };
        match self {
            Self::Bars => match prefs.hidden {
                None => key::FOOTER_SETTINGS_BARS_SHOWN,
                Some(Hidden::Push) => key::FOOTER_SETTINGS_BARS_PUSH,
                Some(Hidden::Cover) => key::FOOTER_SETTINGS_BARS_COVER,
            },
            Self::Hours => on(prefs.hours),
            Self::Kiosk => on(prefs.adapts(crate::kiosk::ADAPTER)),
        }
    }

    /// The sentence under its row, when it has one.
    fn note(self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match self {
            // THE TAP, taught with its keys as an ARGUMENT — the leader and
            // the constant beside the grafted row (`keys-are-never-text`).
            Self::Bars => Some(texts.t_with(
                key::FOOTER_SETTINGS_BARS_NOTE,
                &[(
                    "keys",
                    &format!(
                        "{} {}",
                        crate::bindings::label(),
                        crate::leader::BARS_TAP_KEY
                    ),
                )],
            )),
            Self::Hours => Some(texts.t(key::FOOTER_SETTINGS_HOURS_NOTE)),
            Self::Kiosk => None,
        }
    }
}

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row that acts.
    Up,
    /// Down one row that acts.
    Down,
    /// Change the setting under the cursor.
    Enter,
    /// Close.
    Escape,
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
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
        "footer-settings-down" => Press::Down,
        "footer-settings-up" => Press::Up,
        "footer-settings-enter" => Press::Enter,
        _ => return None,
    })
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Change this setting: the footer's door runs its command, the window
    /// writes the file itself ([`change_the_file`]).
    Change(Setting),
}

/// What writing one change into `modeline.json` came to, for the door that
/// writes it and has to say so.
#[derive(Debug)]
pub enum Changed {
    /// Written: the file is this now.
    Written(Prefs),
    /// The file does not parse, and nothing was written over it: writing the
    /// whole value back would replace what a hand put there.
    Unparsed,
    /// It could not be written — why.
    NotWritten(std::io::Error),
}

/// One setting changed in the file — read with `parsed`, never `read`,
/// changed by [`Setting::change`], written whole and atomically. The window's
/// write; the footer's is `Footer::write_the_modeline`, which also moves its
/// bars.
#[must_use]
pub fn change_the_file(store: &ronoctua_store::Store<Prefs>, setting: Setting) -> Changed {
    let Some(mut now) = store.parsed() else {
        return Changed::Unparsed;
    };
    setting.change(&mut now);
    match store.write(&now) {
        Ok(()) => Changed::Written(now),
        Err(e) => Changed::NotWritten(e),
    }
}

/// One drawn row.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// The setting it changes — only on a row's first line.
    pub setting: Option<Setting>,
    /// A title, a sentence, or the rest of a label: the cursor skips it.
    pub heading: bool,
}

/// The view: where the cursor is. Everything it draws is read from the
/// preferences each door hands it, so the two doors draw one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    cursor: usize,
}

impl View {
    /// Opens it, on the first row that acts.
    #[must_use]
    pub fn fresh(prefs: &Prefs, columns: u16, texts: &ronoctua_i18n::Texts) -> Self {
        let mut me = Self::default();
        let rows = me.rows(prefs, columns, texts);
        me.settle_on(&rows);
        me
    }

    /// Where the cursor is, for the door that draws a band.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line, generated.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// Puts the cursor where a click landed. Answers whether the cursor was
    /// ALREADY on that row: that is the second click, which the door turns
    /// into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            self.cursor = n;
            return again;
        }
        false
    }

    /// The sentence the pointer resting on row `n` reads — the command's
    /// own, which is what `RET` there does.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let setting = rows.get(n)?.setting?;
        Some(texts.t(&format!("cmd-{}", setting.command())))
    }

    /// The rows to draw, at this width, from the preferences as they are.
    #[must_use]
    pub fn rows(&self, prefs: &Prefs, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let end = left + room;
        // ONE COLUMN FOR THE WHOLE LIST, after its widest label: every label
        // counts, whatever its value says today.
        let pairs: Vec<(u16, u16, u16)> = Setting::ALL
            .iter()
            .map(|s| {
                (
                    left,
                    ronoctua_render::layout::width(&texts.t(s.label())),
                    ronoctua_render::layout::width(&texts.t(s.value(prefs))),
                )
            })
            .collect();
        let column = crate::chrome::section_column(left, room, &pairs);
        let mut out: Vec<Row> = Vec::new();
        for setting in Setting::ALL {
            // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
            if !out.is_empty() {
                out.push(Row {
                    fields: crate::chrome::AIR,
                    setting: None,
                    heading: true,
                });
            }
            out.push(Row {
                fields: vec![piece(
                    left,
                    texts.t(setting.heading()),
                    tokens::color::TEXT_HEADING,
                    room,
                )],
                setting: None,
                heading: true,
            });
            let focused = self.cursor == out.len();
            let lines = crate::chrome::aligned_pair(
                left,
                column,
                end,
                &texts.t(setting.label()),
                &texts.t(setting.value(prefs)),
                crate::chrome::Kind::Setting,
                focused,
            );
            for (n, fields) in lines.into_iter().enumerate() {
                out.push(Row {
                    fields,
                    setting: (n == 0).then_some(setting),
                    heading: n > 0,
                });
            }
            // THE NOTE WRAPS, and is never cut (`chrome::note_lines`).
            if let Some(note) = setting.note(texts) {
                for line in crate::chrome::note_lines(&note, room) {
                    out.push(Row {
                        fields: vec![piece(left, line, tokens::color::TEXT_DIM, room)],
                        setting: None,
                        heading: true,
                    });
                }
            }
        }
        out
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows that were DRAWN, and how many rows a wrapped label or note
    /// takes follows the width.
    pub fn press(
        &mut self,
        p: Press,
        prefs: &Prefs,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(prefs, columns, texts);
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
                .and_then(|r| r.setting)
                .map_or(Errand::Nothing, Errand::Change),
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
            if rows.get(at).is_some_and(|r| !r.heading) {
                self.cursor = at;
                return;
            }
        }
    }

    /// Puts the cursor on a row that acts — never on a title.
    pub fn settle_on(&mut self, rows: &[Row]) {
        if rows.get(self.cursor).is_some_and(|r| !r.heading) {
            return;
        }
        self.cursor = rows.iter().position(|r| !r.heading).unwrap_or(0);
    }
}

/// How long until the wall clock's next minute, from `now` since the epoch —
/// what the free minibuffer's hours wait for before they are drawn again.
/// Never zero: a timer at the very edge of a minute would redraw the same one.
#[must_use]
pub fn until_the_next_minute(now: std::time::Duration) -> std::time::Duration {
    let into = std::time::Duration::from_nanos(
        u64::try_from(now.as_nanos() % 60_000_000_000).unwrap_or(0),
    );
    let left = std::time::Duration::from_secs(60).saturating_sub(into);
    if left.is_zero() {
        std::time::Duration::from_secs(60)
    } else {
        left
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

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn rows_of(v: &View, prefs: &Prefs, language: &str) -> Vec<Row> {
        v.rows(prefs, 80, &texts(language))
    }

    /// FAILS IF: a setting has no row that acts, a row that acts is not the
    /// first line of its setting, or the cursor is born on a title.
    #[test]
    fn every_setting_is_a_row_the_cursor_reaches() {
        let t = texts("en-GB");
        let prefs = Prefs::default();
        let v = View::fresh(&prefs, 80, &t);
        let rows = v.rows(&prefs, 80, &t);
        let acting: Vec<Setting> = rows.iter().filter_map(|r| r.setting).collect();
        assert_eq!(acting, Setting::ALL);
        assert!(!rows[v.cursor()].heading, "the cursor was born on a title");
        assert_eq!(rows[v.cursor()].setting, Some(Setting::Bars));
    }

    /// FAILS IF: `RET` on a row changes anything but its own setting — whose
    /// command is the one the footer's door runs — the arrows stop on a
    /// title, or `Esc` does not close.
    #[test]
    fn the_keys_walk_the_settings_and_run_their_commands() {
        let t = texts("en-GB");
        let prefs = Prefs::default();
        let mut v = View::fresh(&prefs, 80, &t);
        let mut seen = Vec::new();
        for _ in Setting::ALL {
            match v.press(Press::Enter, &prefs, 80, &t) {
                Errand::Change(s) => seen.push(s.command()),
                other => panic!("`RET` did {other:?}"),
            }
            assert_eq!(v.press(Press::Down, &prefs, 80, &t), Errand::Nothing);
        }
        assert_eq!(
            seen,
            [crate::hidden::CYCLE_COMMAND, HOURS_COMMAND, KIOSK_COMMAND]
        );
        // ROUND THE END, back on the first.
        let rows = v.rows(&prefs, 80, &t);
        assert_eq!(rows[v.cursor()].setting, Some(Setting::Bars));
        v.press(Press::Up, &prefs, 80, &t);
        let rows = v.rows(&prefs, 80, &t);
        assert_eq!(rows[v.cursor()].setting, Some(Setting::Kiosk));
        assert_eq!(v.press(Press::Escape, &prefs, 80, &t), Errand::Close);
    }

    /// FAILS IF: a row's value stops saying what the file says — each of the
    /// bars' three modes, the hours on and off (off by default), the kiosk's
    /// adapter on by default — or a variant has no words in a language.
    #[test]
    fn every_value_says_what_the_file_says_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut prefs = Prefs::default();
            assert_eq!(
                Setting::Hours.value(&prefs),
                key::FOOTER_SETTINGS_OFF,
                "the hours are off until asked for"
            );
            assert_eq!(Setting::Kiosk.value(&prefs), key::FOOTER_SETTINGS_ON);
            for (hidden, word) in [
                (None, key::FOOTER_SETTINGS_BARS_SHOWN),
                (Some(Hidden::Push), key::FOOTER_SETTINGS_BARS_PUSH),
                (Some(Hidden::Cover), key::FOOTER_SETTINGS_BARS_COVER),
            ] {
                prefs.hidden = hidden;
                assert_eq!(Setting::Bars.value(&prefs), word);
                let rows = rows_of(&View::default(), &prefs, language);
                let drawn: String = rows
                    .iter()
                    .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(drawn.contains(&t.t(word)), "{language} {hidden:?}: {drawn}");
                assert!(!drawn.contains('['), "{language}: a missing key: {drawn}");
            }
            prefs.hours = true;
            assert_eq!(Setting::Hours.value(&prefs), key::FOOTER_SETTINGS_ON);
            prefs.adapt.insert(crate::kiosk::ADAPTER.to_owned(), false);
            assert_eq!(Setting::Kiosk.value(&prefs), key::FOOTER_SETTINGS_OFF);
        }
    }

    /// FAILS IF: the values of the section stop sharing one column, a label
    /// is cut rather than wrapped at a narrow width, or a value is cut on a
    /// screen wide enough for the section.
    #[test]
    fn the_values_share_one_column_and_labels_wrap() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for hidden in [None, Some(Hidden::Push), Some(Hidden::Cover)] {
                let prefs = Prefs {
                    hidden,
                    ..Prefs::default()
                };
                for columns in [30, 60, 120] {
                    let rows = View::default().rows(&prefs, columns, &t);
                    let starts: std::collections::BTreeSet<u16> = rows
                        .iter()
                        .filter(|r| r.setting.is_some())
                        .filter_map(|r| r.fields.get(1).map(|f| f.at))
                        .collect();
                    assert!(starts.len() <= 1, "{language} at {columns}: {starts:?}");
                    for r in &rows {
                        // THE LABEL IS NEVER CUT, at any width: it wraps.
                        if let Some(label) = r.fields.first() {
                            assert!(
                                !label.text.contains('…'),
                                "{language} at {columns}: `{}` cut",
                                label.text
                            );
                        }
                        // AND FROM SIXTY COLUMNS UP, NOTHING IS.
                        if columns >= 60 {
                            for f in &r.fields {
                                assert!(
                                    !f.text.contains('…'),
                                    "{language} at {columns}: `{}` cut",
                                    f.text
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// FAILS IF: the hours' timer waits past the next minute, waits nothing
    /// at the edge of one, or does not land on the minute.
    #[test]
    fn the_hours_wait_for_the_next_minute() {
        use std::time::Duration;
        let at = |s: u64, ms: u64| until_the_next_minute(Duration::from_millis(s * 1000 + ms));
        assert_eq!(
            at(120, 0),
            Duration::from_secs(60),
            "on the edge: a whole one"
        );
        assert_eq!(at(125, 0), Duration::from_secs(55));
        assert_eq!(at(179, 500), Duration::from_millis(500));
        for s in [0, 1, 59, 61, 3599, 86_399] {
            let now = Duration::from_secs(s) + Duration::from_millis(250);
            let next = now + until_the_next_minute(now);
            assert_eq!(next.as_millis() % 60_000, 0, "{s}");
            assert!(until_the_next_minute(now) <= Duration::from_secs(60));
        }
    }

    /// FAILS IF: a change stops walking its setting round — the bars through
    /// their three modes and back, the two switches there and back — or
    /// touches another setting's field.
    #[test]
    fn every_change_walks_its_setting_round() {
        let mut prefs = Prefs::default();
        let mut modes = Vec::new();
        for _ in 0..3 {
            Setting::Bars.change(&mut prefs);
            modes.push(prefs.hidden);
        }
        assert_eq!(modes, [Some(Hidden::Push), Some(Hidden::Cover), None]);
        assert!(!prefs.hours && prefs.adapts(crate::kiosk::ADAPTER));
        Setting::Hours.change(&mut prefs);
        assert!(prefs.hours && prefs.hidden.is_none() && prefs.adapts(crate::kiosk::ADAPTER));
        Setting::Hours.change(&mut prefs);
        assert!(!prefs.hours);
        Setting::Kiosk.change(&mut prefs);
        assert!(!prefs.adapts(crate::kiosk::ADAPTER) && !prefs.hours);
        Setting::Kiosk.change(&mut prefs);
        assert_eq!(
            prefs,
            Prefs {
                adapt: [(crate::kiosk::ADAPTER.to_owned(), true)]
                    .into_iter()
                    .collect(),
                ..Prefs::default()
            }
        );
    }

    /// FAILS IF: the window's write stops saying what it came to — a written
    /// file not answered with what it holds now, a file that does not parse
    /// written over (the hand's text lost), or a directory that will not take
    /// the file answered as written. The last is the owner's bug: the bars'
    /// mode lost at every reboot, and nothing on the screen.
    #[test]
    fn the_windows_write_says_what_it_came_to() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-footer-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let store = ronoctua_store::Store::<Prefs>::at(Some(root.clone()), crate::adapt::FILE);
        // WRITTEN, merged over what is there: the hours a hand turned on stay.
        std::fs::write(root.join(crate::adapt::FILE), br#"{"hours":true}"#).expect("seed");
        match change_the_file(&store, Setting::Bars) {
            Changed::Written(now) => {
                assert_eq!(now.hidden, Some(Hidden::Push));
                assert!(now.hours, "a field another hand set was lost");
                assert_eq!(store.parsed(), Some(now));
            }
            other => panic!("a writable file answered {other:?}"),
        }
        // A FILE THAT DOES NOT PARSE is left as it is.
        let broken = br#"{"hidden":"Cover""#;
        std::fs::write(root.join(crate::adapt::FILE), broken).expect("break it");
        assert!(matches!(
            change_the_file(&store, Setting::Hours),
            Changed::Unparsed
        ));
        assert_eq!(
            std::fs::read(root.join(crate::adapt::FILE)).expect("still there"),
            broken
        );
        // A DIRECTORY THAT WILL NOT TAKE IT: a file where the directory
        // should be refuses as root too, which a read-only mode would not.
        let blocked = root.join("blocked");
        std::fs::write(&blocked, b"").expect("a file in the way");
        let nowhere = ronoctua_store::Store::<Prefs>::at(Some(blocked), crate::adapt::FILE);
        assert!(matches!(
            change_the_file(&nowhere, Setting::Kiosk),
            Changed::NotWritten(_)
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a command a row runs, or a key of the view, is not in the
    /// registry — a row whose `RET` would fall into a log line.
    #[test]
    fn every_command_of_the_view_is_registered() {
        for id in reached() {
            assert!(
                crate::leader::BASE.commands.iter().any(|c| c.id == id),
                "`{id}` is not registered"
            );
        }
        for (_, id) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "`{id}` is no press");
        }
    }
}
