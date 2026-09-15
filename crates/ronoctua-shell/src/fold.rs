//! The minibuffer's line, and how it OPENS.
//!
//! # What it is for
//!
//! The line holds one line of text. When the text is longer than the screen it
//! is cut, and until now the rest was simply unreachable — the owner asked for
//! it from a photograph of the file manager's hint, which is four sentences
//! long and stops mid-word on any real screen.
//!
//! So the line grows a control on its right edge, and opening it shows the
//! rest: the text wrapped over several lines, with a scrollbar when there is
//! still more. Not a popup and not a card (invariants 14 and 15) — it is the
//! same surface growing, the way the notification band grows upwards and the
//! expansion grows downwards.
//!
//! # Why the open height is a CONSTANT
//!
//! This is the decision the whole module is shaped around, and it is not
//! taste. The footer's height is the same on every output
//! (`Footer::apply_geometry`), so a height that followed the wrap would be a
//! height that followed the WIDTH: moving the focus from the laptop to the
//! monitor would reflow every buffer on one of them, with nothing having
//! changed in them. [`crate::band::rows`] makes the same argument about the
//! notification, and answers it the same way — the band's extra lines are the
//! BODY and the ACTIONS, structure rather than wrapping.
//!
//! Here the answer is [`tokens::space::MODELINE_FOLDLINES`]: opened, the line
//! takes that many lines whatever the text and whatever the width. What does
//! not fit in them scrolls, and that is the whole job of the bar. A narrower
//! screen wraps to more lines and therefore scrolls further; it never grows
//! the footer by even one row.
//!
//! # Why it is not a `files::Section`
//!
//! Because the line is not the manager's. It carries the manager's hint, and
//! it also carries an echo, a description of what the pointer is on, and what
//! is being typed into `M-x` — and `M-x` has no sections at all. In the footer
//! it lives in a band of its own between the minibuffer and the brain, which
//! is not the expanded region either.
//!
//! What it does share is the arithmetic: the bar comes from
//! [`ronoctua_design::Scroll`], like every other bar in this system.

use ronoctua_tokens as tokens;

/// What the keyboard asked of the opened line.
///
/// It travels out of `files::View` as a `Step`, because the view does not draw
/// this line and so cannot answer for it: the line is the DOOR's, and the
/// manager only holds the focus that reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Open it, or close it.
    Toggle,
    /// Walk it by this many wrapped lines.
    By(i32),
}

/// Whether the line is open, and how far down it is scrolled.
///
/// One per door. It is not on `files::View` because the line is not the
/// manager's — see this module's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fold {
    /// Is it open?
    pub open: bool,
    /// Which wrapped line is the first one showing.
    pub offset: usize,
    /// How many rows the band was opened AT, and keeps until it closes.
    ///
    /// `0` means "not settled yet": the wrap is only known inside the frame,
    /// and the height is asked for before it — see [`settle`].
    ///
    /// # Why the height stops following the content
    ///
    /// Because a height that follows the content is a height that follows the
    /// LINE, and the line changes under the pointer. The owner found the loop
    /// and described every turn of it: with the drawer open, resting on
    /// something that explains itself put a short message on the line, the
    /// band shrank to fit it, everything below moved UP by the difference, the
    /// row that was under the pointer moved out from under it, the message
    /// went away, the band grew back, the content moved DOWN, the row arrived
    /// under the pointer again — and round.
    ///
    /// Nothing in that chain is wrong on its own. What is wrong is that the
    /// HEIGHT of a band is downstream of what the pointer is resting on. So
    /// the band is measured once, when it opens, and keeps that height until
    /// it closes: a message shorter than the band is drawn inside it with
    /// blank rows under it, and a message longer than it scrolls, which is
    /// what the bar is already for.
    ///
    /// The owner asked for exactly this — *"o espaço de múltiplas linhas deve
    /// continuar como está, e dentro desse espaço será mostrada a mensagem"*.
    pub held: u16,
}

impl Fold {
    /// Opens it, or closes it.
    ///
    /// Closing forgets where it was scrolled to. Opening the same sentence
    /// twice and finding it half way down is a surprise, and there is nothing
    /// to be gained by remembering: the text is short enough to have been one
    /// line's worth of promise.
    pub const fn toggle(&mut self) {
        self.open = !self.open;
        self.offset = 0;
        // AND THE HEIGHT IS ASKED AGAIN. Opening measures the line as it is
        // now; closing forgets, so the next opening is not the last one's
        // size.
        self.held = 0;
    }

    /// Closes it, and says whether that changed anything.
    ///
    /// Called when the line's CONTENT changes: a message that arrives while
    /// the drawer is open would otherwise be read inside a drawer opened over
    /// a different sentence.
    pub const fn close(&mut self) -> bool {
        let was = self.open;
        self.open = false;
        self.offset = 0;
        self.held = 0;
        was
    }
}

/// How many lines the opened band takes, under the line itself. `0` while it
/// is folded.
///
/// `wrapped` is how many rows the text came to at the width it is drawn at;
/// one of them is the minibuffer's own line, so the band under it is one
/// fewer. The ceiling is [`tokens::space::MODELINE_FOLDLINES`], and past it
/// the text scrolls — which is where the bar comes from.
///
/// # THE HEIGHT IS THE CONTENT'S, MEASURED ONCE
///
/// It was a constant, on the argument this module's header still makes: the
/// footer's height is the same on every output, so a height that follows the
/// wrap follows the WIDTH, and moving the focus between monitors of different
/// widths reflows the buffers on one of them. The owner saw the constant on
/// screen — a short sentence opening into a drawer with four empty rows — and
/// chose the other trade: the drawer is the size of what is in it.
///
/// Then he found what "follows the content" costs when the content is the
/// LINE, which changes under the pointer: a layout loop, described in full on
/// [`Fold::held`]. So the measurement happens once, at [`settle`], and this
/// reads the answer back. The cost of the first trade is paid and the loop is
/// not: the height still fits what was opened, and nothing the pointer does
/// changes it.
#[must_use]
pub const fn rows(fold: Fold) -> u16 {
    if !fold.open {
        return 0;
    }
    fold.held
}

/// Measures the opened band ONCE, from the wrap the painter came to.
///
/// Answers whether the height changed, which is what tells the door to ask the
/// compositor for a new size — see the doors' settling pass. It is a no-op
/// while the band is shut, and a no-op once it has an answer: that is what
/// makes it a measurement rather than a feedback path.
///
/// `wrapped` is how many rows the text came to at the width it is drawn at;
/// one of them is the line itself, so the band under it is one fewer. The
/// ceiling is [`tokens::space::MODELINE_FOLDLINES`], and past it the text
/// scrolls — which is where the bar comes from.
pub fn settle(fold: &mut Fold, wrapped: usize) -> bool {
    if !fold.open || fold.held > 0 || wrapped == 0 {
        return false;
    }
    let under = wrapped.saturating_sub(1);
    let want = u16::try_from(under)
        .unwrap_or(u16::MAX)
        .min(tokens::space::MODELINE_FOLDLINES);
    // A BAND OF NO ROWS IS NOT OPEN. A line that fits has no control to press,
    // so this can only be reached by a line that shrank after opening — and
    // one row is what keeps the drawer, and its bar, addressable.
    fold.held = want.max(1);
    true
}

/// How much of the line the control takes, in cells. `0` when there is none.
///
/// The glyph plus the air between it and the text. It is the same shape as the
/// sidebar's eject glyph, which reserves its cells out of the name's room
/// before eliding — a control drawn on top of text that was measured without
/// it is a control that covers a word.
pub const WIDTH: u16 = 2;

/// A bound on the work of wrapping, in lines.
///
/// The text on this line is a sentence — a hint, a message, a path. This is
/// not a limit anybody should reach; it is there so that an echo carrying
/// something pathological cannot make the wrapper walk a megabyte to answer a
/// question about six lines. See [`ronoctua_render::layout::wrap`].
const CEILING: usize = 512;

/// A word of the line, with everything a piece of it carries.
///
/// The register is its colour and the id is which shortcut it belongs to —
/// both travel word by word through the wrap, because the wrap is what
/// decides which ROW each word lands on and both answers are per-row.
type Word = (String, crate::chrome::Register, Option<&'static str>);

/// One thing the wrapper may not break.
enum Unit {
    /// A WHOLE SHORTCUT: its dot, its key and the words that say what it does.
    ///
    /// It is one unit because a key without its words teaches nothing, and the
    /// owner photographed exactly that — `C-<` closing a line and `the first`
    /// opening the next, four times on one screen.
    Entry(Vec<Word>),
    /// A word of prose, which breaks wherever it has to.
    Word(Word),
}

/// The pieces, grouped into the things that may not be broken.
///
/// [`crate::chrome::hint_of_with`] emits a shortcut as a RUN of two or three
/// pieces — `[separator?, key, prose]` — and a separator is what starts the
/// next one. That run is the unit. Everything else is prose: the sentence a
/// message carries, the `type to filter` the manager puts in front of its
/// keys, and a line that is not a hint at all.
fn units_of(pieces: &[crate::chrome::Piece]) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    let mut group: Vec<&crate::chrome::Piece> = Vec::new();
    let close = |group: &mut Vec<&crate::chrome::Piece>, out: &mut Vec<Unit>| {
        if group.is_empty() {
            return;
        }
        // ALL THREE KEY REGISTERS, and the third was missing for a release.
        // `Contextual` had just been added for the keys that exist only while
        // their context does, and a group whose key was one of them was not
        // recognised as a shortcut — so `C-;` and its words went back to being
        // wrapped word by word, which is the exact defect this function was
        // rewritten to end, reappearing on the keys added last.
        let keyed = group.iter().any(|p| {
            matches!(
                p.register,
                crate::chrome::Register::Key
                    | crate::chrome::Register::Alternate
                    | crate::chrome::Register::Contextual
            )
        });
        let words: Vec<Word> = group
            .iter()
            .flat_map(|p| {
                p.text
                    .split_whitespace()
                    .map(move |w| (w.to_owned(), p.register, p.id))
            })
            .collect();
        if keyed {
            out.push(Unit::Entry(words));
        } else {
            out.extend(words.into_iter().map(Unit::Word));
        }
        group.clear();
    };
    for p in pieces {
        if matches!(p.register, crate::chrome::Register::Separator) {
            close(&mut group, &mut out);
        }
        group.push(p);
    }
    close(&mut group, &mut out);
    out
}

/// How wide a run of words is once [`rebuild`] has joined them with spaces.
fn span_of(words: &[Word]) -> usize {
    let ink: usize = words
        .iter()
        .map(|(w, _, _)| usize::from(ronoctua_render::layout::width(w)))
        .sum();
    ink + words.len().saturating_sub(1)
}

/// Puts one word down, breaking the line before it when it does not fit.
fn lay(
    out: &mut Vec<Vec<Word>>,
    line: &mut Vec<Word>,
    used: &mut usize,
    room: usize,
    (word, register, id): Word,
) {
    let w = usize::from(ronoctua_render::layout::width(&word));
    if w > room && line.is_empty() {
        // A WORD WIDER THAN THE LINE — a path, which is what this line
        // carries most. By CLUSTER, for the reason `layout::wrap` gives.
        let mut rest: Vec<&str> = ronoctua_render::layout::cells_of(&word).collect();
        while rest.len() > room && out.len() < CEILING {
            let piece: String = rest.drain(..room).collect();
            out.push(vec![(piece, register, id)]);
        }
        line.push((rest.into_iter().collect::<String>(), register, id));
        *used = usize::from(ronoctua_render::layout::width(&line[0].0));
        return;
    }
    let needs = if line.is_empty() { w } else { w + 1 };
    if *used + needs > room && !line.is_empty() {
        out.push(std::mem::take(line));
        *used = 0;
    }
    *used += if line.is_empty() { w } else { w + 1 };
    line.push((word, register, id));
}

/// The line's text, wrapped over several lines, KEEPING its pieces.
///
/// # Why it wraps pieces and not a string
///
/// Because the line is five registers — a fixed key, a key that follows the
/// focused section, a key that only exists in this context, the prose around
/// them and the mark between one shortcut and the next — and
/// `chrome::hint_spans` is what knows their colours. Wrapping the joined text
/// would give the drawer back as one grey run, so the same sentence would be
/// drawn one way on the first line and another way three lines below it.
///
/// # And why it wraps ENTRIES and not words
///
/// It wrapped words, and the owner photographed what that does: the key at the
/// end of one line and the words that say what it means at the start of the
/// next, four times on one screen. A key and its sentence are ONE THING to
/// read, so they are one thing to place — [`units_of`] recovers the runs that
/// `hint_of_with` emitted, and a break may only fall between them.
///
/// An entry wider than the whole line goes back to breaking by word. That is
/// the pathological case, not the common one, and a line that cannot hold one
/// shortcut has nothing better to offer.
///
/// Spacing is rebuilt as single spaces, exactly as
/// [`ronoctua_render::layout::wrap`] does with a plain text: a wrap is already
/// a rewrite of where the words are.
#[must_use]
pub fn wrap_pieces(pieces: &[crate::chrome::Piece], width: u16) -> Vec<Vec<crate::chrome::Piece>> {
    let room = usize::from(width.max(1));
    let mut out: Vec<Vec<Word>> = Vec::new();
    let mut line: Vec<Word> = Vec::new();
    let mut used = 0_usize;
    for unit in units_of(pieces) {
        // THE CEILING BOUNDS THE WORK, not the layout: the caller windows what
        // it can show. It is here so that a pathological message cannot make
        // this walk a megabyte to answer a question about six lines — the same
        // bound `layout::wrap` carries.
        if out.len() >= CEILING {
            return rebuild(out);
        }
        match unit {
            Unit::Entry(words) => {
                let wide = span_of(&words);
                let needs = if line.is_empty() { wide } else { wide + 1 };
                if wide <= room {
                    if used + needs > room && !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    // AND THE DOT NEVER BEGINS A LINE. It is the mark BETWEEN
                    // two shortcuts, so at the left margin it separates the
                    // entry from nothing.
                    let words: Vec<_> = if line.is_empty() {
                        words
                            .into_iter()
                            .skip_while(|(_, r, _)| matches!(r, crate::chrome::Register::Separator))
                            .collect()
                    } else {
                        words
                    };
                    let wide = span_of(&words);
                    used += if line.is_empty() { wide } else { wide + 1 };
                    line.extend(words);
                    continue;
                }
                // AN ENTRY WIDER THAN THE WHOLE LINE breaks by word — but
                // never between the key and its FIRST word. `C-d` alone on a
                // row is the photographed defect in miniature, and a row
                // reading `C-d permissions` teaches something.
                let mut rest = words.into_iter();
                let mut head: Vec<Word> = Vec::new();
                for word in rest.by_ref() {
                    let prose = matches!(word.1, crate::chrome::Register::Prose);
                    head.push(word);
                    if prose {
                        break;
                    }
                }
                let wide = span_of(&head);
                if used + wide + usize::from(!line.is_empty()) > room && !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    used = 0;
                }
                let head: Vec<_> = if line.is_empty() {
                    head.into_iter()
                        .skip_while(|(_, r, _)| matches!(r, crate::chrome::Register::Separator))
                        .collect()
                } else {
                    head
                };
                let wide = span_of(&head);
                used += wide + usize::from(!line.is_empty());
                line.extend(head);
                for word in rest {
                    lay(&mut out, &mut line, &mut used, room, word);
                }
            }
            Unit::Word(word) => lay(&mut out, &mut line, &mut used, room, word),
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    // AND BACK INTO PIECES, merging neighbours of the same register: a run of
    // prose is one piece, and the door draws one span for it.
    //
    // THE JOINING SPACE GOES ON THE PIECE BEFORE, and that is not a detail: a
    // key piece has to be exactly the notation, because that is what the
    // colour is about. Carried at the front of the next piece, `C-m` would be
    // ` C-m` and the run drawn as a key would start one cell early.
    rebuild(out)
}

/// Turns the wrapped words back into pieces, one row at a time.
fn rebuild(rows: Vec<Vec<Word>>) -> Vec<Vec<crate::chrome::Piece>> {
    rows.into_iter()
        .map(|words| {
            let mut row: Vec<crate::chrome::Piece> = Vec::new();
            for (word, register, id) in words {
                match row.last_mut() {
                    // THE ID JOINS THE REGISTER IN DECIDING WHAT MERGES.
                    // Two runs of prose from two different shortcuts are one
                    // colour and two answers, and merged into one piece the
                    // pointer would be told about whichever of them came
                    // first. It cannot happen while a dot separates every
                    // entry — but the dot is dropped at the left margin, and
                    // "cannot happen today" is not a rule.
                    Some(last) if last.register == register && last.id == id => {
                        last.text.push(' ');
                        last.text.push_str(&word);
                    }
                    Some(last) => {
                        last.text.push(' ');
                        row.push(crate::chrome::Piece {
                            text: word,
                            register,
                            id,
                        });
                    }
                    None => row.push(crate::chrome::Piece {
                        text: word,
                        register,
                        id,
                    }),
                }
            }
            row
        })
        .collect()
}

/// The wrapped line, the window of it that shows, and the bar's arithmetic.
///
/// # Why the window includes the FIRST line
///
/// Because the drawer is the same sentence continuing, not a second copy of
/// it. Drawn as a band below an unchanged summary, the first line came out
/// twice — the owner would have read the same eight words on two rows. So the
/// summary line is the window's first row: opening replaces the elided line
/// with the un-elided one and the rest follows underneath.
#[must_use]
pub fn opened(
    pieces: &[crate::chrome::Piece],
    columns: u16,
    fold: Fold,
) -> (Vec<Vec<crate::chrome::Piece>>, ronoctua_design::Scroll) {
    let all = wrap_pieces(pieces, columns);
    // THE SUMMARY LINE COUNTS, which is the `+ 1`.
    let lines = usize::from(rows(fold)) + 1;
    // THE OFFSET IS CLAMPED HERE and not by whoever moves it, because the
    // number of wrapped lines changes with the width: a window resized
    // narrower under an open drawer leaves the offset past the end, and the
    // honest place to notice is where both numbers are known.
    let offset = fold.offset.min(all.len().saturating_sub(lines));
    let scroll = ronoctua_design::Scroll {
        total: all.len(),
        lines,
        offset,
        top: 0,
    };
    let showing = all.into_iter().skip(offset).take(lines).collect();
    (showing, scroll)
}

/// The sentence that teaches the keys, for the END of an opened hint.
///
/// # Why it is only shown while the line is OPEN
///
/// The owner asked for it in those words, and it is the honest place: the
/// keys walk a line that has been opened, so on a folded line they name a
/// gesture that does nothing. The control's own glyph is what teaches the
/// FIRST step; these teach the ones after it.
///
/// # Why it is appended rather than standing on its own
///
/// Because the hint line is the only thing on screen that teaches a key with
/// no entry in the registry and no line in the manual — the lesson `C-o` cost
/// this project — and a second line saying so would be a second place to look.
#[must_use]
pub fn teaching(texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
    let keys = texts.t(ronoctua_i18n::key::MINIBUFFER_FOLD_KEYS);
    let marked = texts.t_marked(
        ronoctua_i18n::key::MINIBUFFER_FOLD_HINT,
        &[("keys", keys.as_str())],
    );
    let mut out = Vec::new();
    // THROUGH THE SAME DOOR the hint's own sentences come through, so the
    // notation is drawn in the key's colour and the prose in the prose's.
    crate::chrome::Piece::extend(&mut out, &marked, &[keys.as_str()]);
    out
}

/// Does this text need the control at all?
///
/// It does when it would be cut — which is the same question `elide_end` asks,
/// asked before the cut rather than after it. A line that fits has no rest to
/// show, and a control offered where it does nothing is the shape
/// `files::eject_room` already refuses.
#[must_use]
pub fn wanted(text: &str, columns: u16) -> bool {
    ronoctua_render::layout::width(text) > columns
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose(text: &str) -> Vec<crate::chrome::Piece> {
        vec![crate::chrome::Piece::prose(text)]
    }

    /// A drawer opened at the size the given text wraps to, the way a door
    /// does it: measure, then settle.
    fn open_over(pieces: &[crate::chrome::Piece], columns: u16) -> Fold {
        let mut fold = Fold::default();
        fold.toggle();
        settle(&mut fold, wrap_pieces(pieces, columns).len());
        fold
    }

    #[test]
    fn the_drawer_is_the_size_of_what_is_in_it() {
        // THE OWNER'S SECOND CHOICE, made from the screen: a short sentence
        // opening into a drawer with four empty rows was worse than the cost
        // of a height that follows the content.
        let text = prose("one two three four");
        let fold = open_over(&text, 12);
        let (showing, scroll) = opened(&text, 12, fold);
        assert_eq!(scroll.total, 2, "four short words wrap to two rows");
        assert_eq!(showing.len(), 2, "and the drawer shows exactly those");
        assert_eq!(rows(fold), 1, "one row under the line");
        assert!(!scroll.cut(), "and no bar, because nothing was cut");
    }

    #[test]
    fn the_height_is_measured_once_and_the_pointer_cannot_move_it() {
        // THE LOOP THE OWNER FOUND, as an assertion. With the drawer open,
        // resting on something that explains itself put a SHORT message on the
        // line; the band shrank to fit it, everything below moved up, the row
        // left the pointer, the message went away, the band grew back — and
        // round for ever.
        //
        // The band is measured when it OPENS and keeps that height, so a short
        // message is drawn inside it and the rows below do not move.
        let hint = prose(&"word ".repeat(30));
        let fold = open_over(&hint, 20);
        let tall = rows(fold);
        assert!(tall > 1, "the fixture has to open to more than one row");

        // The pointer puts a one-line message on the same line.
        let message = prose("copied");
        assert_eq!(
            rows(fold),
            tall,
            "a short message shrank the band, and everything below it moved"
        );
        let (showing, scroll) = opened(&message, 20, fold);
        // ONE ROW OF CONTENT inside a band that is still `tall` rows deep: the
        // rest are blank, which is what the owner asked for in as many words.
        assert_eq!(showing.len(), 1);
        assert!(!scroll.cut(), "a short message has nothing to scroll");

        // AND SETTLING AGAIN CHANGES NOTHING, which is what makes it a
        // measurement rather than a feedback path.
        let mut again = fold;
        assert!(!settle(&mut again, 1), "the height was asked a second time");
        assert_eq!(again, fold);
    }

    #[test]
    fn it_stops_at_the_ceiling_and_the_bar_takes_over() {
        let long = prose(&"word ".repeat(200));
        let fold = open_over(&long, 20);
        let (showing, scroll) = opened(&long, 20, fold);
        assert_eq!(
            rows(fold),
            tokens::space::MODELINE_FOLDLINES,
            "past the ceiling it stops growing"
        );
        assert_eq!(
            showing.len(),
            usize::from(tokens::space::MODELINE_FOLDLINES) + 1
        );
        assert!(scroll.cut(), "and the rest scrolls");
    }

    #[test]
    fn a_narrower_screen_wraps_to_more_and_may_grow_to_the_ceiling() {
        // The cost of the owner's choice, stated as an assertion: the height
        // follows the width now. It is bounded by the ceiling and it is only
        // paid while the drawer is open.
        let text = prose("a b c d e f g h i j k l m n o p q r s t u v w x y z");
        let (_, wide) = opened(&text, 200, open_over(&text, 200));
        let (_, narrow) = opened(&text, 10, open_over(&text, 10));
        assert!(narrow.total > wide.total, "a narrow screen wraps to more");
        assert!(
            rows(open_over(&text, 10)) >= rows(open_over(&text, 200)),
            "and is at least as tall"
        );
        assert!(
            rows(open_over(&text, 10)) <= tokens::space::MODELINE_FOLDLINES,
            "never past the ceiling"
        );
    }

    #[test]
    fn the_bar_shows_only_when_there_is_more() {
        let brief = prose("one two three");
        let (_, short) = opened(&brief, 40, open_over(&brief, 40));
        assert!(!short.cut(), "three words do not need a bar");
        let long = prose(&"word ".repeat(200));
        let (_, tall) = opened(&long, 20, open_over(&long, 20));
        assert!(tall.cut(), "two hundred words do");
    }

    #[test]
    fn the_offset_is_clamped_against_the_width_of_the_moment() {
        // A window resized narrower under an open drawer: the offset was
        // valid for the old wrap and is past the end of the new one.
        let text = prose("one two three four");
        let fold = Fold {
            offset: 400,
            ..open_over(&text, 40)
        };
        let (showing, scroll) = opened(&text, 40, fold);
        assert_eq!(scroll.offset, 0);
        assert!(!showing.is_empty(), "the drawer came out empty");
    }

    #[test]
    fn a_closed_line_takes_no_lines_at_all() {
        assert_eq!(rows(Fold::default()), 0);
    }

    #[test]
    fn the_control_is_offered_only_where_it_does_something() {
        assert!(!wanted("short", 40));
        assert!(wanted("a sentence longer than the room it was given", 10));
    }

    #[test]
    fn closing_forgets_where_it_was() {
        let mut fold = Fold {
            open: true,
            offset: 3,
            held: 2,
        };
        assert!(fold.close());
        assert_eq!(fold, Fold::default());
        assert!(!fold.close(), "closing a closed line changes nothing");
    }

    #[test]
    fn a_key_stays_a_key_after_the_wrap() {
        // The whole reason this wraps PIECES: the drawer draws through the
        // same painter as the line above it, so `C-m` is the same colour on
        // both.
        let line = vec![
            crate::chrome::Piece::prose("type to filter \u{b7} "),
            crate::chrome::Piece::key("C-m", false),
            crate::chrome::Piece::prose(" opens the action menu"),
        ];
        let out = wrap_pieces(&line, 12);
        let keys: Vec<&str> = out
            .iter()
            .flatten()
            .filter(|p| matches!(p.register, crate::chrome::Register::Key))
            .map(|p| p.text.as_str())
            .collect();
        assert_eq!(keys, vec!["C-m"], "the key was lost or split");
        for row in &out {
            let wide: u16 = row
                .iter()
                .map(|p| ronoctua_render::layout::width(&p.text))
                .sum();
            assert!(wide <= 12, "a wrapped row is wider than the room");
        }
    }

    #[test]
    fn the_words_survive_the_wrap() {
        let out = wrap_pieces(&prose("one two three four five"), 9);
        let back: String = out
            .iter()
            .map(|r| {
                r.iter()
                    .map(|p| p.text.as_str())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(back, "one two three four five");
    }

    /// FAILS IF: a row ends on a key, which is a key torn from the words that
    /// say what it does.
    ///
    /// THE OWNER PHOTOGRAPHED IT, four times on one screen: `C-<` closing a
    /// line and `the first` opening the next, and the same for `C-l`, `C-{`
    /// and `C-d`. The wrapper flattened every piece into words, so nothing
    /// anywhere bound a key to its sentence.
    #[test]
    fn a_row_never_ends_on_a_key() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        // THE REAL TABLE, and the widest one this system has: if the manager's
        // thirty-six shortcuts wrap without tearing, so does every other.
        //
        // HALF OF THEM MARKED CONTEXTUAL, because that register was added
        // after this test was written and `units_of` did not know it: a group
        // whose key was one of them was not recognised as a shortcut at all,
        // and went back to being wrapped word by word — the photographed
        // defect, alive again on the keys added last. Marked here, the test
        // sees it.
        let line = crate::chrome::hint_of_marked(
            &[
                ("<down>", "files-down", false),
                ("<up>", "files-up", false),
                ("C-<", "files-first", false),
                ("C->", "files-last", false),
                ("C-l", "files-sidebar", false),
                ("C-{", "files-playlist-previous", true),
                ("C-}", "files-playlist-next", true),
                ("C-d", "files-columns", false),
                ("C-w", "files-theatre", true),
            ],
            |_| None,
            &texts,
        );
        for width in [20u16, 30, 40, 60, 80, 120, 200] {
            let out = wrap_pieces(&line, width);
            for (n, row) in out.iter().enumerate() {
                let Some(last) = row.last() else { continue };
                assert!(
                    !matches!(
                        last.register,
                        crate::chrome::Register::Key
                            | crate::chrome::Register::Alternate
                            | crate::chrome::Register::Contextual
                    ),
                    "at {width} cells, row {n} ends on the key `{}`",
                    last.text
                );
                // AND NO ROW BEGINS ON THE DOT, which separates an entry from
                // nothing at the left margin.
                let Some(first) = row.first() else { continue };
                assert!(
                    !matches!(first.register, crate::chrome::Register::Separator),
                    "at {width} cells, row {n} begins on the separator"
                );
            }
        }
    }

    /// THE SHORTCUT'S ID SURVIVES THE WRAP, on every row.
    ///
    /// The drawer is where the whole table is legible, so it is where somebody
    /// goes looking for what a key does — and the pointer's answer there comes
    /// from marks taken off the WRAPPED rows. Dropped in `rebuild`, the second
    /// row onwards would explain nothing; carried but merged across entries,
    /// it would explain the wrong key.
    #[test]
    fn a_wrapped_row_still_knows_which_shortcut_each_word_is() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let table: &[(&'static str, &'static str)] = &[
            ("C-v", "files-preview"),
            ("C-i", "files-facts"),
            ("C-l", "files-sidebar"),
            ("C-d", "files-columns"),
            ("C-w", "files-theatre"),
        ];
        let line = crate::chrome::hint_of(table, &texts);
        for width in [24u16, 32, 48, 70, 120] {
            let rows = wrap_pieces(&line, width);
            let mut seen: Vec<&'static str> = Vec::new();
            for row in &rows {
                for p in row {
                    match p.register {
                        crate::chrome::Register::Separator => {
                            assert_eq!(p.id, None, "the dot belongs to no shortcut");
                        }
                        _ => {
                            let id = p.id.unwrap_or_else(|| {
                                panic!("at {width} cells, `{}` lost its shortcut", p.text)
                            });
                            if !seen.contains(&id) {
                                seen.push(id);
                            }
                        }
                    }
                }
            }
            // ALL FIVE, IN THE TABLE'S ORDER: a merge across two entries would
            // swallow one of them, and that is the shape in which this would
            // fail silently.
            assert_eq!(
                seen,
                table.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
                "at {width} cells the wrap lost or reordered a shortcut"
            );
        }
        // AND TWO RUNS OF ONE COLOUR THAT ARE TWO SHORTCUTS STAY TWO PIECES.
        //
        // Built by hand, because the hint's own line cannot reach this shape:
        // a dot of its own register sits between every pair of entries. That
        // is exactly why `rebuild` asks the id as well as the register — a
        // merge here would hand the pointer one answer for two keys, and there
        // would be nothing on screen to say which one it was.
        let two = vec![
            crate::chrome::Piece::prose("first").about("files-preview"),
            crate::chrome::Piece::prose("second").about("files-facts"),
        ];
        let rows = wrap_pieces(&two, 80);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![Some("files-preview"), Some("files-facts")],
            "two shortcuts of one colour were merged into one answer"
        );
    }

    /// FAILS IF: a shortcut that FITS on a line is broken anyway.
    #[test]
    fn a_shortcut_that_fits_is_not_broken() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let line =
            crate::chrome::hint_of(&[("<down>", "files-down"), ("<up>", "files-up")], &texts);
        // Wide enough for one entry and not for two.
        let out = wrap_pieces(&line, 12);
        assert_eq!(out.len(), 2, "two entries did not take two rows: {out:?}");
        for row in &out {
            let said: String = row.iter().map(|p| p.text.as_str()).collect();
            assert!(
                said.split_whitespace().count() >= 2,
                "a row carries a key with no words: {said:?}"
            );
        }
    }
}
