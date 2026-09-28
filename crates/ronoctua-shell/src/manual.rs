//! The manuals — `LEADER h` lists them, and each opens from the list, in the
//! footer or in a window of its own. The first, and the reason this file
//! exists, is the SHORTCUTS'.
//!
//! # Why it exists
//!
//! The chords left the list the leader's tap opens. They were not choices about
//! what one is doing there — the leader is already released, and pressing `←`
//! at that moment does nothing —, so they took twenty-five lines to say what
//! exists in another mode of use. The tap's list halved, and what left needed a
//! place.
//!
//! # Why it is not a file
//!
//! A manual written by hand is the second copy of the shortcut table, and the
//! day it lies is the day someone trusts it. Here the lines come out of
//! [`crate::actions::chords`] and [`crate::actions::emergency`] at the moment
//! of opening: changing a shortcut changes the manual, with no intermediate
//! step. It is the same reason the compositor's binds are generated.
//!
//! # Two doors, one [`View`]
//!
//! The list opens **in place of the shortcut list**, in the footer's expanded
//! region — the fourth use of the same surface, beside which-key, the `M-x`
//! and the history — and in a window of its own (`ronoctua-shell manual`,
//! `manual_window.rs`). Both doors drive the same [`View`]: the list with its
//! filter typed in place, a manual opened from it, `Esc` one level up, and
//! the same keys and clicks on each level. A manual opened in the footer is
//! taken into a window with `C-w`.
//!
//! A manual is read-only by construction, and not by a lock: what is typed
//! narrows the LIST, and there is no input path that writes to a manual.

use ronoctua_i18n::{Texts, key};
use ronoctua_keymap::{Key, Sym};

/// The manuals there are — the list `LEADER h` opens. A manual has NO key
/// of its own (the owner's rule): it is chosen from this list, where `RET`
/// opens it where the list is and `C-w` in a window of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The shortcuts: the chords, the compositor's modes, the emergency and
    /// media keys.
    Shortcuts,
    /// The hand gestures, drawn.
    Gestures,
}

impl Which {
    /// Every manual, in the list's order.
    pub const ALL: [Self; 2] = [Self::Shortcuts, Self::Gestures];

    /// Its name in the list.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shortcuts => key::MANUAL_SHORTCUTS,
            Self::Gestures => key::MANUAL_GESTURES,
        }
    }

    /// What the list says about it under the pointer.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Shortcuts => key::ABOUT_MANUAL_SHORTCUTS,
            Self::Gestures => key::ABOUT_MANUAL_GESTURES,
        }
    }

    /// The word the window is opened with: `ronoctua-shell manual <word>`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Shortcuts => "shortcuts",
            Self::Gestures => "gestures",
        }
    }

    /// The manual a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.word() == word)
    }

    /// Its lines, in the active language.
    #[must_use]
    pub fn lines(self, texts: &Texts, chord: &str) -> Vec<Line> {
        match self {
            Self::Shortcuts => aired(assemble(texts, chord)),
            Self::Gestures => aired(gestures(texts)),
        }
    }
}

/// The keys of the LIST of manuals, as the hint line teaches them. `ESC` is
/// not here: it leaves one level everywhere, and the line does not teach a
/// key that is the same in every view.
pub const OF_THE_LIST: &[(&str, &str)] = &[
    ("<down>", "manual-next"),
    ("<up>", "manual-previous"),
    ("RET", "manual-open"),
    ("C-w", "manual-window"),
];

/// And of an OPEN manual.
pub const OF_A_MANUAL: &[(&str, &str)] = &[
    ("<down>", "manual-down"),
    ("<up>", "manual-up"),
    ("C-w", "manual-window"),
];

/// The command id of `C-w`: a manual taken out of the footer into a window.
/// A window has no row of it — it is already the window.
pub const TO_A_WINDOW: &str = "manual-window";

/// Where the manuals are: their list, or one of them open — the view both
/// doors drive. What each door keeps is its own page offset and nothing
/// about the manuals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    /// The manual open, or `None` for the list.
    which: Option<Which>,
    /// The list's cursor, in what the filter left.
    cursor: usize,
    /// What has been typed to filter the list.
    query: String,
}

/// How an open manual moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    /// So many lines — the arrows.
    Lines(i32),
    /// So many pages — `SPC`, `<next>` and `<prior>`.
    Pages(i32),
    /// To the first line — `<home>`.
    Top,
    /// To the last — `<end>`.
    Bottom,
}

/// What a key or a click asks of the door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Nothing changed.
    Nothing,
    /// The list changed — its filter or its cursor: draw it again.
    Changed,
    /// The level changed — a manual opened, or the list came back: the page
    /// goes back to its top.
    Level,
    /// The open manual moves.
    Scroll(Scroll),
    /// This manual, in a window of its own (`C-w`).
    Window(Which),
    /// The clipboard's text is wanted for the filter (`C-v`, `C-y`).
    Paste,
    /// The view closes.
    Close,
}

impl View {
    /// The list, with nothing typed.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// A manual already open — `ronoctua-shell manual <word>`, and the
    /// footer's `C-w`. `Esc` goes up to the list, as it does in the footer.
    #[must_use]
    pub fn on(which: Which) -> Self {
        Self {
            which: Some(which),
            ..Self::default()
        }
    }

    /// The manual open, or `None` for the list.
    #[must_use]
    pub const fn which(&self) -> Option<Which> {
        self.which
    }

    /// The list's cursor, in what the filter left.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// What has been typed to filter the list.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The keys this level teaches.
    #[must_use]
    pub const fn taught(&self) -> &'static [(&'static str, &'static str)] {
        if self.which.is_some() {
            OF_A_MANUAL
        } else {
            OF_THE_LIST
        }
    }

    /// The manuals the query leaves, best first.
    #[must_use]
    pub fn found(&self, texts: &Texts) -> Vec<Which> {
        let names: Vec<String> = Which::ALL.iter().map(|w| texts.t(w.name())).collect();
        let keys: Vec<[&str; 1]> = names.iter().map(|n| [n.as_str()]).collect();
        let items: Vec<&[&str]> = keys.iter().map(|k| &k[..]).collect();
        crate::minibuffer::rank(&self.query.to_lowercase(), &items)
            .into_iter()
            .map(|i| Which::ALL[i])
            .collect()
    }

    /// Which letters of `name` the query matched — to light them.
    #[must_use]
    pub fn lit(&self, name: &str) -> Vec<usize> {
        crate::minibuffer::match_up(&self.query.to_lowercase(), name)
            .map(|(_, p)| p)
            .unwrap_or_default()
    }

    /// The manual `C-w` takes into a window: the open one, or the one under
    /// the list's cursor.
    fn chosen(&self, texts: &Texts) -> Option<Which> {
        self.which
            .or_else(|| self.found(texts).get(self.cursor).copied())
    }

    /// One key — the same in both doors.
    ///
    /// On the LIST: the arrows walk it, `RET` opens the manual under the
    /// cursor where the list is, a letter or `SPC` is typed into the filter,
    /// `DEL` takes one cluster off it, `C-v` and `C-y` paste into it, and
    /// `Esc` closes. On a MANUAL: the arrows move it a line, `SPC` and the page
    /// keys a page, `<home>` and `<end>` to its ends, and `Esc` goes back to
    /// the list. `C-w` takes either into a window.
    pub fn key(&mut self, key: Key, texts: &Texts) -> Errand {
        let chord = key.mods.ctrl || key.mods.alt || key.mods.logo;
        if key.mods.ctrl && key.sym == Sym::Char('w') {
            return self.chosen(texts).map_or(Errand::Nothing, Errand::Window);
        }
        if key.sym == Sym::Esc {
            return if self.which.take().is_some() {
                Errand::Level
            } else {
                Errand::Close
            };
        }
        if self.which.is_some() {
            return match key.sym {
                Sym::Down => Errand::Scroll(Scroll::Lines(1)),
                Sym::Up => Errand::Scroll(Scroll::Lines(-1)),
                Sym::PageDown | Sym::Space => Errand::Scroll(Scroll::Pages(1)),
                Sym::PageUp => Errand::Scroll(Scroll::Pages(-1)),
                Sym::Home => Errand::Scroll(Scroll::Top),
                Sym::End => Errand::Scroll(Scroll::Bottom),
                // Nothing else touches it. A manual is read-only, and a key
                // that "almost" works teaches that there is something to type.
                _ => Errand::Nothing,
            };
        }
        match key.sym {
            Sym::Char('v' | 'y') if key.mods.ctrl => Errand::Paste,
            _ if chord => Errand::Nothing,
            Sym::Down => {
                let last = self.found(texts).len().saturating_sub(1);
                self.cursor = (self.cursor + 1).min(last);
                Errand::Changed
            }
            Sym::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Errand::Changed
            }
            Sym::Enter => self.open_here(texts),
            // THE QUERY, a cluster at a time — the minibuffer's rule.
            Sym::Backspace => {
                if crate::line::pop_cluster(&mut self.query) {
                    self.cursor = 0;
                    Errand::Changed
                } else {
                    Errand::Nothing
                }
            }
            // A SPACE IS TYPED: the manuals' names have spaces in them
            // ("Hand gestures"), and a filter that could not type one could
            // not type a name.
            Sym::Space => self.typed(" "),
            // A CONTROL CHARACTER IS NOT TYPED (`Delete`'s U+007F).
            Sym::Char(c) if !c.is_control() => self.typed(c.encode_utf8(&mut [0; 4])),
            _ => Errand::Nothing,
        }
    }

    /// Text into the filter, and the cursor back on the best match.
    fn typed(&mut self, text: &str) -> Errand {
        self.query.push_str(text);
        self.cursor = 0;
        Errand::Changed
    }

    /// What was copied, into the filter — as ONE line (`line::one_line`).
    pub fn pasted(&mut self, text: &str) -> Errand {
        if self.which.is_some() {
            return Errand::Nothing;
        }
        let line = crate::line::one_line(text);
        if line.is_empty() {
            return Errand::Nothing;
        }
        self.typed(&line)
    }

    /// Opens the manual under the list's cursor, here.
    fn open_here(&mut self, texts: &Texts) -> Errand {
        match self.found(texts).get(self.cursor).copied() {
            Some(which) => {
                self.which = Some(which);
                Errand::Level
            }
            None => Errand::Nothing,
        }
    }

    /// A click on manual `n` of the list: THE FIRST SELECTS, AND THE SECOND
    /// OPENS it (invariant 19) — `RET` on the cursor, by the same path.
    pub fn clicked(&mut self, n: usize, texts: &Texts) -> Errand {
        if self.which.is_some() || n >= self.found(texts).len() {
            return Errand::Nothing;
        }
        if self.cursor == n {
            return self.open_here(texts);
        }
        self.cursor = n;
        Errand::Changed
    }
}

/// How a line of a manual is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A section's title.
    Heading,
    /// A shortcut: its key, and what it does.
    Plain,
    /// WHAT A GESTURE DOES — the line the eye looks for, so it is the one
    /// drawn loudest.
    Function,
    /// How the hands are held for it.
    Pose,
    /// The rest of what there is to know about it.
    Detail,
    /// A line with nothing on it — the AIR before a heading, which a heading
    /// glued to the content above it does not have: hierarchy here is colour
    /// and air (invariant 15), and the owner read the missing air off the
    /// screen.
    Air,
}

/// One line of a manual.
pub struct Line {
    /// How the combination is written — `SUPER + ←`. Empty on a gesture's
    /// lines, whose left column is the drawing.
    pub key: String,
    /// What it says, in the active language.
    pub description: String,
    /// How it is drawn.
    pub tone: Tone,
    /// The gesture DRAWN to the left of this line and the ones under it
    /// ([`ronoctua_gestures::art::picture`]): set on a gesture's first line.
    pub picture: Option<&'static str>,
    /// Whether this line belongs to a gesture, and so has the drawing's
    /// column on its left.
    pub beside_picture: bool,
}

impl Line {
    /// A section header.
    #[must_use]
    pub fn heading(description: String) -> Self {
        Self {
            key: String::new(),
            description,
            tone: Tone::Heading,
            picture: None,
            beside_picture: false,
        }
    }

    /// A shortcut.
    #[must_use]
    pub const fn plain(key: String, description: String) -> Self {
        Self {
            key,
            description,
            tone: Tone::Plain,
            picture: None,
            beside_picture: false,
        }
    }

    /// A line of air.
    #[must_use]
    pub const fn air() -> Self {
        Self {
            key: String::new(),
            description: String::new(),
            tone: Tone::Air,
            picture: None,
            beside_picture: false,
        }
    }

    /// Whether it is a section header.
    #[must_use]
    pub fn title(&self) -> bool {
        self.tone == Tone::Heading
    }
}

/// How wide a manual's LEFT column is, in cells: its widest key, or the
/// drawings' column when it has drawings. Both doors measure it here, so the
/// text they wrap is wrapped to the room they draw it in.
#[must_use]
pub fn key_column(lines: &[Line]) -> u16 {
    lines
        .iter()
        .map(|l| {
            if l.beside_picture {
                ronoctua_tokens::space::GESTURES_PICTURECELLS
            } else {
                ronoctua_render::layout::width(&l.key)
            }
        })
        .max()
        .unwrap_or(0)
}

/// The manual as a door `columns` wide draws it: every line WRAPPED to the
/// room its column leaves, never cut — a manual is read whole, and what an
/// ellipsis takes is always the end of a sentence (the owner's rule; the
/// same one `chrome::note_lines` keeps for notes).
///
/// `lead` is how many cells the door puts before the text column besides the
/// key's own — its colon, its gaps and its bar; a HEADING takes the whole
/// width less `edge`. A wrapped line's continuation keeps the tone and the
/// drawing's column, and no key and no drawing of its own.
#[must_use]
pub fn laid_out(lines: Vec<Line>, columns: u16, lead: u16, edge: u16) -> Vec<Line> {
    let key = key_column(&lines);
    let text_room = columns.saturating_sub(key + lead).max(8);
    let heading_room = columns.saturating_sub(edge).max(8);
    let mut out = Vec::with_capacity(lines.len());
    for l in lines {
        if l.tone == Tone::Air {
            out.push(l);
            continue;
        }
        let room = if l.title() { heading_room } else { text_room };
        let mut pieces =
            ronoctua_render::layout::wrap(&l.description, room, usize::MAX).into_iter();
        let first = pieces.next().unwrap_or_default();
        let (tone, beside) = (l.tone, l.beside_picture);
        out.push(Line {
            description: first,
            ..l
        });
        for rest in pieces {
            out.push(Line {
                key: String::new(),
                description: rest,
                tone,
                picture: None,
                beside_picture: beside,
            });
        }
    }
    out
}

/// A line of air before every heading but the first — see [`Tone::Air`].
fn aired(lines: Vec<Line>) -> Vec<Line> {
    let mut out = Vec::with_capacity(lines.len() + 8);
    for l in lines {
        if l.title() && !out.is_empty() {
            out.push(Line::air());
        }
        out.push(l);
    }
    out
}

/// Assembles the whole manual, in the active language.
///
/// `chord` is how the chords' key appears — `SUPER` in production, `ALT` in
/// the nested session. It comes from outside because whoever knows that is
/// whoever defines the bind (see [`crate::leader_chord`]), and not this module.
#[must_use]
pub fn assemble(texts: &Texts, chord: &str) -> Vec<Line> {
    let mut out = vec![Line::heading(texts.t(key::HELP_CHORDS))];

    // THE CHORDS, in the table's order — which is the order in which they make
    // sense together (focus, move, window, spaces, open), and not alphabetical.
    let nested = crate::actions::nested();
    for a in crate::actions::full_chords() {
        out.push(Line::plain(
            format!("{chord} + {}", a.shows_for(nested)),
            texts.t(&a.label()),
        ));
    }

    // THE COMPOSITOR'S MODES, each under its own title: the chord that opens
    // it is above with the others, and what the keys do once inside is here —
    // the only place besides the hint line a mode draws while it is on.
    for mode in crate::actions::modes() {
        out.push(Line::heading(texts.t(&format!(
            "{}{}",
            ronoctua_cmd::KEY_PREFIX_OF,
            mode.deed
        ))));
        for k in &mode.keys {
            out.push(Line::plain(
                k.shows.clone(),
                texts.t(&format!("{}{}", ronoctua_cmd::KEY_PREFIX_OF, k.command)),
            ));
        }
        out.push(Line::plain(
            "ESC".to_owned(),
            texts.t(&format!("{}wm-mode-default", ronoctua_cmd::KEY_PREFIX_OF)),
        ));
    }

    // THE EMERGENCY BINDS. They are what one looks for when the leader does not
    // answer — and that is why the manual needs them more than any other line. A
    // manual that only documents the happy path fails exactly when it is
    // consulted.
    out.push(Line::heading(texts.t(key::HELP_EMERGENCY)));
    for (key, _, command) in crate::actions::emergency() {
        let description = if command == crate::actions::LEADER_ALTERNATE {
            texts.t(key::HELP_LEADER_ALTERNATE)
        } else {
            texts.t(&format!("{}{command}", ronoctua_cmd::KEY_PREFIX_OF))
        };
        out.push(Line::plain(key.to_owned(), description));
    }

    // THE KEYBOARD'S TOP ROW, for the emergency binds' reason: a key that is
    // bound and appears in no list is a key nobody can find out about. These
    // are the only bare binds besides those four, and they are the ones a
    // person is most likely to press without ever reading anything.
    out.push(Line::heading(texts.t(key::HELP_MEDIA)));
    for (key, _, command) in crate::actions::media() {
        out.push(Line::plain(
            key.to_owned(),
            texts.t(&format!("{}{command}", ronoctua_cmd::KEY_PREFIX_OF)),
        ));
    }

    out
}

/// How many lines a gesture takes, and so how tall its drawing is: what it
/// does, how the hands are held, and the rest.
pub const GESTURE_LINES: usize = 3;

// THE DRAWING IS AS TALL AS THE LINES, and the token and this number are two
// spellings of one fact: the compiler holds them together.
const _: () = assert!(GESTURE_LINES == ronoctua_tokens::space::GESTURES_PICTURELINES as usize);

/// The gestures' manual, in the active language: every row of
/// [`ronoctua_gestures::GESTURES`] as three lines beside its DRAWING — what it
/// does (the loudest), how the hands are held, and the rest. Generated at
/// opening for the reason the shortcuts' manual is: a gesture added to the
/// reader is a picture and three lines here with no step in between, and one
/// missing a sentence or a picture fails a test below.
#[must_use]
pub fn gestures(texts: &Texts) -> Vec<Line> {
    let mut out = vec![Line::heading(texts.t(key::HELP_GESTURES))];
    for g in ronoctua_gestures::GESTURES {
        let line = |description: String, tone: Tone, picture: Option<&'static str>| Line {
            key: String::new(),
            description,
            tone,
            picture,
            beside_picture: true,
        };
        out.push(line(
            texts.t(&format!("gesture-{g}-title")),
            Tone::Function,
            Some(g),
        ));
        out.push(line(texts.t(&format!("gesture-{g}")), Tone::Pose, None));
        out.push(line(
            texts.t(&format!("gesture-{g}-does")),
            Tone::Detail,
            None,
        ));
        // AND AIR BETWEEN ONE GESTURE AND THE NEXT: a drawing with three lines
        // beside it, glued to the next one's, read as one long block.
        if Some(g) != ronoctua_gestures::GESTURES.last() {
            out.push(Line::air());
        }
    }
    out
}

/// The box a gesture's drawing is fitted in, in logical pixels: as wide as
/// the manual's picture column and as tall as a gesture's lines.
#[must_use]
pub fn gesture_art_box() -> (f32, f32) {
    let size = ronoctua_tokens::text::size();
    (
        f32::from(ronoctua_tokens::space::GESTURES_PICTURECELLS)
            * ronoctua_tokens::metrics::cell_width(size),
        f32::from(ronoctua_tokens::space::GESTURES_PICTURELINES)
            * ronoctua_tokens::metrics::line_height(size),
    )
}

/// The key of a gesture's drawing: the gesture, the size and the theme — the
/// ink is `currentColor` resolved in THIS theme, and a key without it would
/// keep the old ink on the GPU after a switch (`buffer::svg_key`'s lesson).
#[must_use]
pub fn gesture_art_key(id: &str) -> String {
    let (w, h) = gesture_art_box();
    format!(
        "gesture:{id}:{w:.0}x{h:.0}:{}",
        ronoctua_tokens::theme::palette()
    )
}

/// A gesture's drawing, rasterised to fit its box, in the manual's ink.
#[must_use]
pub fn gesture_art(id: &str) -> Option<ronoctua_render::image::Rgba> {
    let svg = ronoctua_gestures::art::picture(id)?;
    let [r, g, b, _] = ronoctua_tokens::color::TEXT_SECONDARY.to_rgba8();
    let svg = svg.replace("currentColor", &format!("#{r:02X}{g:02X}{b:02X}"));
    let (w, h) = gesture_art_box();
    // THE DRAWING IS TWICE AS WIDE AS TALL: the side that fits is the smaller
    // of the box's width and twice its height. `Natural` and not `Fill`:
    // `Fill` answers a SQUARE, which squeezed into this box drew every
    // gesture at half its height — measured on the owner's screen.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let side = w.min(2.0 * h).floor() as u32;
    let r =
        ronoctua_notify::icon::from_svg(svg.as_bytes(), side, ronoctua_notify::icon::Fit::Natural)?;
    Some(ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    })
}

#[cfg(test)]
mod tests {
    use ronoctua_i18n::Texts;
    use ronoctua_keymap::Key;

    fn en() -> Texts {
        Texts::fresh("en-GB").expect("en-GB")
    }

    fn key(v: &mut super::View, notation: &str, texts: &Texts) -> super::Errand {
        v.key(Key::parse(notation).expect("a key"), texts)
    }

    fn typed(v: &mut super::View, text: &str, texts: &Texts) {
        for c in text.chars() {
            let notation = if c == ' ' {
                "SPC".to_owned()
            } else {
                c.to_string()
            };
            key(v, &notation, texts);
        }
    }

    /// WHAT IS TYPED NARROWS THE LIST, a space included — the manuals' names
    /// have spaces, and the footer's filter could not type one — and `DEL`
    /// takes a cluster back off.
    #[test]
    fn the_list_is_narrowed_by_what_is_typed_a_space_included() {
        use super::{Errand, View, Which};
        let t = en();
        let mut v = View::fresh();
        assert_eq!(v.found(&t).len(), Which::ALL.len());
        typed(&mut v, "hand g", &t);
        assert_eq!(v.query(), "hand g");
        assert_eq!(v.found(&t), vec![Which::Gestures]);
        assert_eq!(key(&mut v, "DEL", &t), Errand::Changed);
        assert_eq!(v.query(), "hand ");
        // NOTHING LEFT, AND `RET` OPENS NOTHING.
        typed(&mut v, "zzz", &t);
        assert!(v.found(&t).is_empty());
        assert_eq!(key(&mut v, "RET", &t), Errand::Nothing);
        assert_eq!(v.which(), None);
    }

    /// `Esc` GOES UP ONE LEVEL: out of a manual to the list, out of the list
    /// closed — in both doors, which drive this same view.
    #[test]
    fn esc_goes_up_one_level_and_then_closes() {
        use super::{Errand, View, Which};
        let t = en();
        let mut v = View::fresh();
        assert_eq!(key(&mut v, "<down>", &t), Errand::Changed);
        assert_eq!(key(&mut v, "RET", &t), Errand::Level);
        assert_eq!(v.which(), v.found(&t).get(1).copied());
        assert_eq!(key(&mut v, "ESC", &t), Errand::Level);
        assert_eq!(v.which(), None);
        assert_eq!(key(&mut v, "ESC", &t), Errand::Close);
        let mut opened = View::on(Which::Gestures);
        assert_eq!(key(&mut opened, "ESC", &t), Errand::Level);
        assert_eq!(
            opened.which(),
            None,
            "a manual opened by its word goes up to the list"
        );
    }

    /// AN OPEN MANUAL MOVES BY A LINE, A PAGE OR TO ITS ENDS, and the page
    /// keys are keys of both doors now: the footer had no symbol for them.
    #[test]
    fn a_manual_moves_by_lines_pages_and_to_its_ends() {
        use super::{Errand, Scroll, View, Which};
        let t = en();
        let mut v = View::on(Which::Shortcuts);
        for (notation, how) in [
            ("<down>", Scroll::Lines(1)),
            ("<up>", Scroll::Lines(-1)),
            ("SPC", Scroll::Pages(1)),
            ("<next>", Scroll::Pages(1)),
            ("<prior>", Scroll::Pages(-1)),
            ("<home>", Scroll::Top),
            ("<end>", Scroll::Bottom),
        ] {
            assert_eq!(key(&mut v, notation, &t), Errand::Scroll(how), "{notation}");
        }
        // A LETTER IS NOT TYPED INTO A MANUAL.
        assert_eq!(key(&mut v, "a", &t), Errand::Nothing);
        assert_eq!(v.query(), "");
    }

    /// THE FIRST CLICK SELECTS, AND THE SECOND OPENS — invariant 19.
    #[test]
    fn the_first_click_selects_and_the_second_opens() {
        use super::{Errand, View};
        let t = en();
        let mut v = View::fresh();
        assert_eq!(v.clicked(1, &t), Errand::Changed);
        assert_eq!(v.cursor(), 1);
        assert_eq!(v.which(), None);
        assert_eq!(v.clicked(1, &t), Errand::Level);
        assert_eq!(v.which(), v.found(&t).get(1).copied());
        assert_eq!(
            v.clicked(0, &t),
            Errand::Nothing,
            "a manual's lines take no click"
        );
        let mut v = View::fresh();
        assert_eq!(v.clicked(9, &t), Errand::Nothing, "past the list");
    }

    /// A CHORD IS NOT A LETTER OF THE FILTER, `C-v` pastes into it as one
    /// line, and `C-w` takes the manual under the cursor into a window.
    #[test]
    fn a_chord_is_not_typed_c_v_pastes_and_c_w_is_the_window() {
        use super::{Errand, View};
        let t = en();
        let mut v = View::fresh();
        assert_eq!(key(&mut v, "C-a", &t), Errand::Nothing);
        assert_eq!(key(&mut v, "M-a", &t), Errand::Nothing);
        assert_eq!(v.query(), "");
        assert_eq!(key(&mut v, "C-v", &t), Errand::Paste);
        assert_eq!(v.pasted("  Hand\n"), Errand::Changed);
        assert_eq!(v.query(), "Hand");
        assert_eq!(v.pasted(" \n"), Errand::Nothing);
        let chosen = v.found(&t)[0];
        assert_eq!(key(&mut v, "C-w", &t), Errand::Window(chosen));
        // AND A MANUAL TAKES NO PASTE: nothing is typed into it.
        let mut open = View::on(chosen);
        assert_eq!(open.pasted("x"), Errand::Nothing);
        assert_eq!(key(&mut open, "C-w", &t), Errand::Window(chosen));
    }

    #[test]
    fn every_gesture_has_its_picture_and_its_three_sentences() {
        let texts = Texts::from_env();
        let m = super::gestures(&texts);
        let n = ronoctua_gestures::GESTURES.len();
        assert_eq!(
            m.len(),
            n * super::GESTURE_LINES + (n - 1) + 1,
            "and air between"
        );
        for l in m
            .iter()
            .filter(|l| !l.title() && l.tone != super::Tone::Air)
        {
            assert!(
                !l.description.starts_with('['),
                "a gesture line with no text: {}",
                l.description
            );
        }
        let drawn: Vec<&str> = m.iter().filter_map(|l| l.picture).collect();
        assert_eq!(
            drawn,
            ronoctua_gestures::GESTURES,
            "one drawing each, in order"
        );
        for g in drawn {
            assert!(
                ronoctua_gestures::art::picture(g).is_some(),
                "{g} has no drawing"
            );
        }
    }

    #[test]
    fn every_manual_is_named_and_opens_by_its_word() {
        let texts = Texts::from_env();
        for w in super::Which::ALL {
            assert!(!texts.t(w.name()).starts_with('['));
            assert!(!texts.t(w.about()).starts_with('['));
            assert_eq!(super::Which::of_word(w.word()), Some(w));
        }
    }

    #[test]
    fn every_chord_shows_up_in_the_manual() {
        // THE MANUAL MUST NOT LIE BY OMISSION. It exists because the chords left the
        // tap's list: if one of them does not arrive here, it stops being
        // discoverable anywhere — worse than before.
        let texts = Texts::from_env();
        let m = super::assemble(&texts, "SUPER");
        for a in crate::actions::full_chords() {
            assert!(
                m.iter().any(|l| l.key.ends_with(a.shows)),
                "the chord `{}` does not appear in the manual",
                a.shows
            );
        }
        // And the emergency ones, for the same reason, with even more cause.
        for (key, _, _) in crate::actions::emergency() {
            assert!(
                m.iter().any(|l| l.key == key),
                "the emergency bind `{key}` does not appear in the manual"
            );
        }
        // AND THE MEDIA KEYS. They are bare binds too, and the manual is the
        // only place they are written down — the hint line never teaches
        // them, because they belong to no view.
        for (key, _, _) in crate::actions::media() {
            assert!(
                m.iter().any(|l| l.key == key),
                "the media bind `{key}` does not appear in the manual"
            );
        }
    }

    #[test]
    fn no_row_comes_out_without_text() {
        // A key that does not exist becomes `[key]` on screen (the `t()` does
        // that on purpose, so the error is visible). Here it would be visible
        // LATE: in the manual, which is opened once a month.
        let texts = Texts::from_env();
        for l in super::assemble(&texts, "SUPER") {
            assert!(!l.description.is_empty(), "a row with no description");
            assert!(
                !l.description.starts_with('['),
                "a missing text key: {}",
                l.description
            );
            assert!(l.title() || !l.key.is_empty(), "a shortcut with no key");
        }
    }

    #[test]
    fn a_long_sentence_is_wrapped_and_never_cut() {
        let texts = Texts::from_env();
        let whole = super::Which::Gestures.lines(&texts, "SUPER");
        let narrow = super::laid_out(super::Which::Gestures.lines(&texts, "SUPER"), 50, 4, 1);
        let join = |ls: &[super::Line]| {
            ls.iter()
                .filter(|l| l.tone != super::Tone::Air)
                .map(|l| l.description.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert!(
            narrow.len() > whole.len(),
            "something was wrapped at 50 columns"
        );
        assert_eq!(
            join(&whole).split_whitespace().collect::<Vec<_>>(),
            join(&narrow).split_whitespace().collect::<Vec<_>>(),
            "every word is still there"
        );
        let room = 50 - super::key_column(&narrow) - 4;
        for l in narrow.iter().filter(|l| !l.title()) {
            assert!(
                ronoctua_render::layout::width(&l.description) <= room,
                "{}",
                l.description
            );
        }
        assert_eq!(
            narrow.iter().filter(|l| l.picture.is_some()).count(),
            ronoctua_gestures::GESTURES.len(),
            "a drawing each, never repeated on a continuation"
        );
    }

    #[test]
    fn every_heading_but_the_first_has_air_above_it() {
        let texts = Texts::from_env();
        for w in super::Which::ALL {
            let m = w.lines(&texts, "SUPER");
            for (i, l) in m.iter().enumerate().skip(1) {
                if l.title() {
                    assert_eq!(
                        m[i - 1].tone,
                        super::Tone::Air,
                        "{:?}: {}",
                        w,
                        l.description
                    );
                }
            }
        }
    }

    #[test]
    fn the_chords_label_goes_into_the_key() {
        // The manual is the only place where the chord's key appears written out.
        // In the nested session it is `ALT`, and not `CTRL ALT SPC` — holding Space
        // along is what fires the SEQUENCE, the opposite of the chord.
        let texts = Texts::from_env();
        let m = super::assemble(&texts, "ALT");
        assert!(m.iter().any(|l| l.key.starts_with("ALT + ")));
        assert!(!m.iter().any(|l| l.key.contains("CTRL ALT SPC")));
    }
}
