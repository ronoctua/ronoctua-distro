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
/// # THE HEIGHT FOLLOWS THE CONTENT, and what that costs
///
/// It was a constant, on the argument this module's header still makes: the
/// footer's height is the same on every output, so a height that follows the
/// wrap follows the WIDTH, and moving the focus between monitors of different
/// widths reflows the buffers on one of them. The owner saw the constant on
/// screen — a short sentence opening into a drawer with four empty rows — and
/// chose the other trade: the drawer is the size of what is in it, up to the
/// ceiling.
///
/// So the cost is real and it is bounded: it is paid only while a drawer is
/// OPEN, which is a state somebody entered on purpose and leaves with one
/// key, and never past the ceiling.
#[must_use]
pub fn rows(fold: Fold, wrapped: usize) -> u16 {
    if !fold.open {
        return 0;
    }
    let under = wrapped.saturating_sub(1);
    u16::try_from(under)
        .unwrap_or(u16::MAX)
        .min(tokens::space::MODELINE_FOLDLINES)
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

/// The line's text, wrapped over several lines, KEEPING its pieces.
///
/// # Why it wraps pieces and not a string
///
/// Because the line is two registers — a key notation and the prose around it
/// — and `chrome::hint_spans` is what knows their colours. Wrapping the joined
/// text would give the drawer back as one grey run, so the same sentence would
/// be drawn one way on the first line and another way three lines below it.
///
/// Spacing is rebuilt as single spaces, exactly as
/// [`ronoctua_render::layout::wrap`] does with a plain text: a wrap is already
/// a rewrite of where the words are.
#[must_use]
pub fn wrap_pieces(pieces: &[crate::chrome::Piece], width: u16) -> Vec<Vec<crate::chrome::Piece>> {
    let room = usize::from(width.max(1));
    // ONE FLAT LIST OF WORDS, each remembering which register it came from.
    let mut words: Vec<(String, bool)> = Vec::new();
    for p in pieces {
        for w in p.text.split_whitespace() {
            words.push((w.to_owned(), p.key));
        }
    }
    let mut out: Vec<Vec<(String, bool)>> = Vec::new();
    let mut line: Vec<(String, bool)> = Vec::new();
    let mut used = 0_usize;
    for (word, key) in words {
        let w = usize::from(ronoctua_render::layout::width(&word));
        if w > room && line.is_empty() {
            // A WORD WIDER THAN THE LINE — a path, which is what this line
            // carries most. By CLUSTER, for the reason `layout::wrap` gives.
            let mut rest: Vec<&str> = ronoctua_render::layout::cells_of(&word).collect();
            while rest.len() > room && out.len() < CEILING {
                let piece: String = rest.drain(..room).collect();
                out.push(vec![(piece, key)]);
            }
            line.push((rest.into_iter().collect::<String>(), key));
            used = usize::from(ronoctua_render::layout::width(&line[0].0));
            continue;
        }
        let needs = if line.is_empty() { w } else { w + 1 };
        if used + needs > room && !line.is_empty() {
            out.push(std::mem::take(&mut line));
            used = 0;
            // THE CEILING BOUNDS THE WORK, not the layout: the caller windows
            // what it can show. It is here so that a pathological message
            // cannot make this walk a megabyte to answer a question about six
            // lines — the same bound `layout::wrap` carries.
            if out.len() >= CEILING {
                return rebuild(out);
            }
        }
        used += if line.is_empty() { w } else { w + 1 };
        line.push((word, key));
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
fn rebuild(rows: Vec<Vec<(String, bool)>>) -> Vec<Vec<crate::chrome::Piece>> {
    rows.into_iter()
        .map(|words| {
            let mut row: Vec<crate::chrome::Piece> = Vec::new();
            for (word, key) in words {
                match row.last_mut() {
                    Some(last) if last.key == key => {
                        last.text.push(' ');
                        last.text.push_str(&word);
                    }
                    Some(last) => {
                        last.text.push(' ');
                        row.push(crate::chrome::Piece { text: word, key });
                    }
                    None => row.push(crate::chrome::Piece { text: word, key }),
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
    let lines = usize::from(rows(fold, all.len())) + 1;
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
        vec![crate::chrome::Piece {
            text: text.to_owned(),
            key: false,
        }]
    }

    #[test]
    fn the_drawer_is_the_size_of_what_is_in_it() {
        // THE OWNER'S SECOND CHOICE, made from the screen: a short sentence
        // opening into a drawer with four empty rows was worse than the cost
        // of a height that follows the content.
        let fold = Fold {
            open: true,
            offset: 0,
        };
        let (showing, scroll) = opened(&prose("one two three four"), 12, fold);
        assert_eq!(scroll.total, 2, "four short words wrap to two rows");
        assert_eq!(showing.len(), 2, "and the drawer shows exactly those");
        assert_eq!(rows(fold, scroll.total), 1, "one row under the line");
        assert!(!scroll.cut(), "and no bar, because nothing was cut");
    }

    #[test]
    fn it_stops_at_the_ceiling_and_the_bar_takes_over() {
        let fold = Fold {
            open: true,
            offset: 0,
        };
        let long = "word ".repeat(200);
        let (showing, scroll) = opened(&prose(&long), 20, fold);
        assert_eq!(
            rows(fold, scroll.total),
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
        let fold = Fold {
            open: true,
            offset: 0,
        };
        let text = prose("a b c d e f g h i j k l m n o p q r s t u v w x y z");
        let (_, wide) = opened(&text, 200, fold);
        let (_, narrow) = opened(&text, 10, fold);
        assert!(narrow.total > wide.total, "a narrow screen wraps to more");
        assert!(
            rows(fold, narrow.total) >= rows(fold, wide.total),
            "and is at least as tall"
        );
        assert!(
            rows(fold, narrow.total) <= tokens::space::MODELINE_FOLDLINES,
            "never past the ceiling"
        );
    }

    #[test]
    fn the_bar_shows_only_when_there_is_more() {
        let fold = Fold {
            open: true,
            offset: 0,
        };
        let (_, short) = opened(&prose("one two three"), 40, fold);
        assert!(!short.cut(), "three words do not need a bar");
        let long = "word ".repeat(200);
        let (_, tall) = opened(&prose(&long), 20, fold);
        assert!(tall.cut(), "two hundred words do");
    }

    #[test]
    fn the_offset_is_clamped_against_the_width_of_the_moment() {
        // A window resized narrower under an open drawer: the offset was
        // valid for the old wrap and is past the end of the new one.
        let fold = Fold {
            open: true,
            offset: 400,
        };
        let (showing, scroll) = opened(&prose("one two three four"), 40, fold);
        assert_eq!(scroll.offset, 0);
        assert!(!showing.is_empty(), "the drawer came out empty");
    }

    #[test]
    fn a_closed_line_takes_no_lines_at_all() {
        assert_eq!(rows(Fold::default(), 40), 0);
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
            crate::chrome::Piece {
                text: "type to filter \u{b7} ".to_owned(),
                key: false,
            },
            crate::chrome::Piece {
                text: "C-m".to_owned(),
                key: true,
            },
            crate::chrome::Piece {
                text: " opens the action menu".to_owned(),
                key: false,
            },
        ];
        let out = wrap_pieces(&line, 12);
        let keys: Vec<&str> = out
            .iter()
            .flatten()
            .filter(|p| p.key)
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
}
