//! Exercises the Unicode symbol browser.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its own
//! (invariant 12): a `cargo test` of it would test the logic **outside** the
//! component — precisely what proves nothing. Here it is loaded the way the host
//! loads it, driven by `event` and read by `render`.
//!
//! Usage: `ronoctua-test-symbols <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{Command, Input, Text};

/// How many columns a single-width character occupies at icon scale.
const ICON_COLUMNS: u16 = 2;

/// The scale heights the production host sends. The numbers are the tokens';
/// the ARITHMETIC is the same the host uses.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 60;
const COLUMNS: u16 = 80;

/// How many characters the database has. **Exact**, and not a floor.
///
/// A floor would catch a truncated database; the exact number also catches the opposite —
/// the generator starting to emit what it should not. And this buffer's database has a
/// trap of its own: `}` is INSIDE it (`U+007D`), and the component's parser
/// splits records on `"},"`. If some string field goes back to
/// carrying an arbitrary character, this number changes.
const TOTAL: usize = 11614;

/// The dotted circle a combining mark is drawn on top of.
///
/// Written here on purpose, like the icon token's name in the other driver:
/// it is what ties the two ends together. Whoever removes `shows` from the generator makes this
/// test fail instead of the mark disappearing from the screen silently.
const CIRCLE: char = '\u{25CC}';

/// The prefix the buffer OPENS with.
const INITIAL_FILTER: &str = "family:emoji";

/// What is seen on screen.
struct Vision {
    /// How many results the header says there are.
    filtered: usize,
    /// How many characters there are in total.
    total: usize,
    /// What is written in the search FIELD. `None` when it shows the
    /// invitation (an empty search).
    field: Option<String>,
    /// What was DRAWN in each grid cell, in reading order.
    ///
    /// The whole string, and not the first `char`: for a combining mark the
    /// drawing starts at the dotted circle, and reading only the first would make every
    /// criterion about marks compare against `◌`.
    drawings: Vec<String>,
    /// Where each was drawn, in the same order.
    positions: Vec<(u16, u16)>,
    /// Each drawing's declared budget, in the same order.
    budgets: Vec<u16>,
}

/// Reads the display list: the header on row 0, the grid below, the focus on the last.
fn read(list: &ronoctua_protocol::DisplayList) -> Result<Vision> {
    let mut counter: Option<(u16, String)> = None;
    let mut field: Option<String> = None;
    let mut cells: std::collections::BTreeMap<(u16, u16), (String, u16)> =
        std::collections::BTreeMap::new();

    for c in &list.commands {
        let Command::Text {
            line,
            column,
            text,
            max_width,
            ..
        } = c
        else {
            continue;
        };
        let Text::Data(s) = text else { continue };
        if *line == 0 {
            // ROW 0 HAS TWO DATA RUNS: what is in the field and the
            // counter, flush right. What decides is the COLUMN.
            match &counter {
                Some((col, _)) if *col >= *column => field = Some(s.clone()),
                Some((col, text_before)) => {
                    field = Some(text_before.clone());
                    let _ = col;
                    counter = Some((*column, s.clone()));
                }
                None => counter = Some((*column, s.clone())),
            }
        } else {
            cells.insert((*line, *column), (s.clone(), *max_width));
        }
    }

    // THE LAST ROW HAS THREE RUNS: the focused character, its name, and the
    // tags in whatever is left.
    // THERE IS NO `focus` FIELD HERE, and the absence is deliberate: the focused
    // character's identity comes from the SHEET (`unicode-sheet-codepoint`), never from the
    // drawing in the footer. For a combining mark the drawing starts at the
    // dotted circle, and comparing drawings would give false in 302 cases.
    let last_one = cells.keys().next_back().map(|(l, _)| *l);
    if let Some(l) = last_one {
        let on_line: Vec<(u16, String)> = cells
            .range((l, 0)..=(l, u16::MAX))
            .map(|((_, c), (s, _))| (*c, s.clone()))
            .collect();
        for (column, _) in &on_line {
            cells.remove(&(l, *column));
        }
    }

    let mut drawings = Vec::new();
    let mut positions = Vec::new();
    let mut budgets = Vec::new();
    for (k, (s, w)) in &cells {
        drawings.push(s.clone());
        positions.push(*k);
        budgets.push(*w);
    }

    let (_, cab) = counter.context("`render` emitted no counter on line 0")?;
    let (n, m) = cab
        .split_once('/')
        .with_context(|| format!("the counter is not in the `N / M` shape: {cab}"))?;

    Ok(Vision {
        filtered: n.trim().parse().context("the result count")?,
        total: m.trim().parse().context("the database total")?,
        field,
        drawings,
        positions,
        budgets,
    })
}

fn screen(inst: &mut ronoctua_runtime::Instance) -> Result<Vision> {
    read(&inst.render(COLUMNS, ROWS, heights())?)
}

/// Types a query from scratch.
fn query(inst: &mut ronoctua_runtime::Instance, q: &str) -> Result<Vision> {
    inst.event(&Input::Command("unicode-filter-clear".to_owned()))?;
    inst.event(&Input::Paste(q.to_owned()))?;
    screen(inst)
}

/// The text keys emitted, with the budget the buffer reserved.
fn keys(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Key(k),
                max_width,
                ..
            } => Some((k.clone(), *max_width)),
            _ => None,
        })
        .collect()
}

/// The panel's options: key, row, column and budget. Empty = closed.
fn panel_of(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16, u16, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Key(k),
                max_width,
                ..
            } if k.starts_with("unicode-family-")
                || k.starts_with("unicode-group-")
                || k.starts_with("unicode-tone-") =>
            {
                Some((k.clone(), *line, *column, *max_width))
            }
            _ => None,
        })
        .collect()
}

/// A `.ftl` as a `key -> text` table.
fn ftl(path: &str) -> Result<std::collections::HashMap<String, String>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))?;
    Ok(raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect())
}

/// The value of a sheet row, by label.
fn sheet_value(list: &ronoctua_protocol::DisplayList, label: &str) -> Option<String> {
    let line = list.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            text: Text::Key(k),
            ..
        } if k == label => Some(*line),
        _ => None,
    })?;
    list.commands.iter().find_map(|c| match c {
        Command::Text {
            line: l,
            text: Text::Data(s),
            ..
        } if *l == line => Some(s.clone()),
        _ => None,
    })
}

/// Opens the focused character's sheet and returns its display list.
fn sheet(inst: &mut ronoctua_runtime::Instance) -> Result<ronoctua_protocol::DisplayList> {
    inst.event(&Input::Confirm)?;
    inst.render(COLUMNS, ROWS, heights())
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-symbols <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-symbols <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR: a command outside the keymap is not reachable ----
    let d = inst.init()?;

    // THE PHRASES, exactly as the host hands them over at load. Without this a
    // buffer that holds interface text would be driven against its fallback,
    // and the screen this test reads would not be the screen anybody sees.
    // en-GB is the canonical one; the pt-BR round trip is checked separately.
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;
    if d.mode != "unicode" {
        bail!("the buffer's mode is `{}`; the modeline shows that", d.mode);
    }
    for id in &d.commands {
        if !d.keymap.iter().any(|l| l.command == *id) {
            bail!("`{id}` has no key: nobody reaches it through which-key");
        }
    }
    if let Err(e) = d.validate() {
        bail!("the descriptor does not pass the section 7.9 guard: {e}");
    }
    // EVERY COMMAND HAS A KEY IN BOTH LANGUAGES. Without this which-key shows
    // `[cmd-unicode-copy-utf8]`, which is the usual silent fallback.
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains_key(&key) {
                bail!("`{key}` does not exist in {language}.ftl");
            }
        }
    }
    println!(
        "descriptor {} commands, all with a key and a text key",
        d.commands.len()
    );

    // ---- THE DATABASE ARRIVED WHOLE ----
    //
    // THE DEFECT THIS CATCHES: the component has no JSON parser and splits the
    // records on `"},"`. The glyph browser can afford that luxury
    // because its database starts at `U+E000`; **here `}` is in the database**. What
    // saves it is the generator not emitting a string field with an arbitrary character —
    // and if that changes, the number changes.
    let initial = screen(&mut inst)?;
    if initial.total != TOTAL {
        bail!(
            "the database has {} characters and should have {TOTAL}. If it dropped, some field of the\n\
             generator went back to carrying `}}`, `\"` or `,` and the parser split it down the middle.",
            initial.total
        );
    }

    // ---- IT OPENS FILTERED, AND SAYS SO ----
    //
    // THE DEFECT THIS CATCHES: "opening filtered" done with a hidden
    // flag. The user would see 3.8xx of 7,529 with an empty field and with
    // no clue as to why.
    let field = initial
        .field
        .clone()
        .context("the search field emitted nothing on opening")?;
    if field != INITIAL_FILTER {
        bail!("the buffer opened with the field `{field}`, and not with `{INITIAL_FILTER}`");
    }
    if initial.filtered == 0 || initial.filtered >= TOTAL {
        bail!(
            "the opening showed {} of {TOTAL}: the symbol filter did not narrow",
            initial.filtered
        );
    }
    println!(
        "opening   {} of {TOTAL}, with `{field}` in the field",
        initial.filtered
    );

    // ---- `Esc` IN TWO LAYERS ----
    //
    // A single layer would drop the symbol filter on the first press, by
    // surprise; none would trap whoever wants to reach the letters.
    // WITH A SPACE: the enumeration filter takes one word and the rest is a
    // free term — which is how one types with a filter active.
    inst.event(&Input::Paste(" rosto".to_owned()))?;
    let with_term = screen(&mut inst)?;
    if with_term.filtered >= initial.filtered {
        bail!("the free term did not narrow within the symbol filter");
    }
    inst.event(&Input::Cancel)?;
    let back1 = screen(&mut inst)?;
    if back1.field.as_deref() != Some(INITIAL_FILTER) {
        bail!(
            "the first `Esc` should clear the TERM and keep the filter; it left {:?}",
            back1.field
        );
    }
    inst.event(&Input::Cancel)?;
    let back2 = screen(&mut inst)?;
    if back2.filtered != TOTAL {
        bail!(
            "the second `Esc` should show all {TOTAL}; it showed {}. Without this there is no\n\
             way through to the letters, and the opening filter becomes a trap.",
            back2.filtered
        );
    }
    if !inst.event(&Input::Cancel)?.is_empty() {
        bail!("the third `Esc`, with the search already empty, asked for a redraw for nothing");
    }
    println!("esc       term -> filter -> everything ({TOTAL})");

    // ---- AND THE LETTERS ARE THERE ----
    let greek = query(&mut inst, "GREEK SMALL LETTER ALPHA")?;
    if greek.filtered == 0 {
        bail!("after the double `Esc`, searching for a Greek letter finds nothing");
    }

    // ---- THE SEARCH MATCHES BY WORD, AND IN A PHRASE ----
    //
    // THE TWO DEFECTS THIS CATCHES, both of which the owner saw on screen:
    //
    //   - `seta para cima` returned ZERO. The term was matched WHOLE against the
    //     name and against each tag, and the tags are loose words: none
    //     contains the phrase.
    //   - searching `seta` put `PESETA SIGN` FIRST, because "the name
    //     contains" scores better than "has the tag" and `pe·seta·` matches as
    //     a substring.
    let phrase = query(&mut inst, "seta para cima")?;
    if phrase.filtered == 0 {
        bail!(
            "`seta para cima` found nothing. A phrase has to match when ALL the\n\
             words match — matching the whole phrase against each tag never finds anything,\n\
             because the tags are single words."
        );
    }
    // AND IT RESTRICTS MORE than a single word: more words, fewer results.
    let so_seta = query(&mut inst, "seta")?;
    if phrase.filtered >= so_seta.filtered {
        bail!(
            "`seta para cima` found {} and `seta` found {}: the second word did not\n\
             narrow — it is being ignored.",
            phrase.filtered,
            so_seta.filtered
        );
    }
    // THE ORDER: the first result for `seta` has to BE an arrow.
    //
    // The proof is the focused character's name, and not the count: `PESETA SIGN`
    // is still in the results (it is legitimate, `peseta` starts with those letters), but
    // it must not come ahead of whoever has the tag.
    let f = sheet(&mut inst)?;
    let name = sheet_value(&f, "unicode-sheet-codepoint").unwrap_or_default();
    let first = so_seta.drawings.first().cloned().unwrap_or_default();
    inst.event(&Input::Cancel)?;
    if name == "U+20A7" {
        bail!(
            "searching `seta` put {first:?} (U+20A7 PESETA SIGN) first. A loose substring\n\
             in the name is beating the tag: `pe·seta·` should not match `seta`."
        );
    }
    println!(
        "search    phrase {} · `seta` {} · first {first:?}",
        phrase.filtered, so_seta.filtered
    );

    // ---- THE EMOJI ----
    //
    // The buffer OPENS on them, so the first screen has already shown them. Here we assert
    // what the rest of the database does not have: group, subgroup, and copying the SEQUENCE.
    let emoji = query(&mut inst, "familia:emoji")?;
    if emoji.filtered != 3944 {
        bail!(
            "the `emoji` family has {} records; expected 3,944",
            emoji.filtered
        );
    }
    let emoji_faces = query(&mut inst, "grupo:carinhas")?;
    if emoji_faces.filtered == 0 || emoji_faces.filtered >= emoji.filtered {
        bail!(
            "`grupo:carinhas` did not narrow: {} of {}",
            emoji_faces.filtered,
            emoji.filtered
        );
    }
    // SEARCHING IN PORTUGUESE, coming from the CLDR and not from the lexicon.
    let pt = query(&mut inst, "bandeira brasil")?;
    if pt.filtered == 0 {
        bail!("`bandeira brasil` found nothing — the CLDR keywords did not arrive");
    }
    // COPYING A SEQUENCE carries the WHOLE sequence.
    //
    // THE DEFECT THIS CATCHES: copying only the first codepoint of 👩‍💻 would give
    // 👩 — the truncated emoji. It is the easiest defect to miss, because the
    // screen looks right and what goes into the document is wrong.
    query(&mut inst, "\u{1F469}\u{200D}\u{1F4BB}")?;
    let f = sheet(&mut inst)?;
    let hex = sheet_value(&f, "unicode-sheet-codepoint").unwrap_or_default();
    if !hex.contains("U+200D") {
        bail!("the sheet of 👩‍💻 shows {hex:?}: the sequence was truncated at the first codepoint");
    }
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copying the emoji did not ask for Copy")?;
    if copied.chars().count() != 3 {
        bail!(
            "copying 👩‍💻 gave back {copied:?} with {} characters; the sequence has 3",
            copied.chars().count()
        );
    }
    inst.event(&Input::Cancel)?;
    println!(
        "emoji     {} · faces {} · `bandeira brasil` {}",
        emoji.filtered, emoji_faces.filtered, pt.filtered
    );

    // ---- THE FILTERS RESTRICT ----
    let family = query(&mut inst, "familia:letra")?;
    let category = query(&mut inst, "cat:sm")?;
    let cat_larga = query(&mut inst, "cat:s")?;
    let block = query(&mut inst, "bloco:arrows")?;
    for (name, v) in [
        ("familia:letra", &family),
        ("cat:sm", &category),
        ("bloco:arrows", &block),
    ] {
        if v.filtered == 0 || v.filtered >= TOTAL {
            bail!("`{name}` did not narrow: {} of {TOTAL}", v.filtered);
        }
    }
    // THE CATEGORY MATCHES BY PREFIX: `cat:s` catches the four symbol types, and
    // a half-typed query must not return zero while one is typing.
    if cat_larga.filtered <= category.filtered {
        bail!(
            "`cat:s` gave back {} and `cat:sm` gave back {}: the code is matching exactly\n\
             instead of by prefix",
            cat_larga.filtered,
            category.filtered
        );
    }
    // THE BLOCK IS BY NAME, and not by hexadecimal range: Unicode's block
    // boundaries do not fall on round multiples, and the other browser's shortcut
    // would answer wrong on every one of them.
    let f = sheet(&mut inst)?;
    let focus_block = sheet_value(&f, "unicode-sheet-block").unwrap_or_default();
    if !focus_block.to_lowercase().contains("arrows") {
        bail!("`bloco:arrows` let a character of block `{focus_block}` through");
    }
    inst.event(&Input::Cancel)?;
    println!(
        "filters   family {} · cat:sm {} · cat:s {} · block {}",
        family.filtered, category.filtered, cat_larga.filtered, block.filtered
    );

    // ---- THE 302 COMBINING MARKS ----
    //
    // THE DEFECT THIS CATCHES: a mark drawn on its own takes up no room —
    // it disappears, or paints over the neighbouring cell. The conventional base is the
    // dotted circle, and what puts it there is the GENERATOR (the component cannot
    // write a codepoint, invariants 9 and 11).
    let marks = query(&mut inst, "familia:marca")?;
    if marks.filtered == 0 {
        bail!("there are no combining marks in the database — did the `familia` field go away?");
    }
    let first = marks
        .drawings
        .first()
        .context("the marks' family drew nothing")?;
    if first.chars().count() < 2 || !first.starts_with(CIRCLE) {
        bail!(
            "the combining mark was drawn as {first:?}: without the dotted circle\n\
             it goes away from the cell or paints over its neighbour."
        );
    }
    // AND THE COPY DOES NOT CARRY THE CIRCLE ALONG.
    //
    // It is the worst defect this feature can have, and the only one that
    // survives any visual inspection: the screen looks right and what goes into
    // the user's document is wrong.
    let f = sheet(&mut inst)?;
    let mark_codepoint = sheet_value(&f, "unicode-sheet-codepoint")
        .context("the mark's sheet did not show the codepoint")?;
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copying the mark did not ask for Copy")?;
    if copied.chars().count() != 1 {
        bail!(
            "copying the mark gave back {copied:?}, with {} characters; it should be ONE",
            copied.chars().count()
        );
    }
    if copied.starts_with(CIRCLE) {
        bail!(
            "copying the mark gave back the DOTTED CIRCLE — the support went along into the text"
        );
    }
    let expected = format!("U+{:04X}", copied.chars().next().unwrap_or('?') as u32);
    if expected != mark_codepoint {
        bail!("the sheet says {mark_codepoint} and the copy gave back {expected}");
    }
    inst.event(&Input::Cancel)?;
    inst.event(&Input::Cancel)?;
    println!(
        "marks     {} drawn over `{CIRCLE}`, and the copy comes out clean",
        marks.filtered
    );

    // ---- THE 23 DOUBLE-WIDTH ONES ----
    let wide = query(&mut inst, "LONG LEFTWARDS ARROW")?;
    if wide.filtered == 0 {
        bail!("could not find `LONG LEFTWARDS ARROW` — did the database change?");
    }
    // THE BUDGET FOLLOWS THE INK, and the proof is a PAIR:
    //
    //   - in the query that brings up the long ones, SOME declare more than two columns;
    //   - in the `Arrows` block, where all are single-width, NONE declares.
    //
    // One side alone does not prove it: "LEFTWARDS ARROW" also matches "LONG LEFTWARDS
    // ARROW", so both maxima would give four and the test would pass with the
    // budget fixed at four for everyone — which would elide the rest.
    let largest = wide
        .budgets
        .iter()
        .copied()
        .max()
        .context("the grid did not draw the wide character")?;
    if largest <= ICON_COLUMNS {
        bail!(
            "none of the long characters declared more than {ICON_COLUMNS} columns.\n\
             The host will elide a single character into `…`, out of its own budget."
        );
    }
    // `box drawing`, and not `arrows`: the block filter matches on the name's
    // CONTENT, so `arrows` also brings `Supplemental Arrows-A`, which is where
    // the long arrows live. The box drawing block has none.
    let simple = query(&mut inst, "bloco:box drawing")?;
    if simple.budgets.iter().any(|o| *o != ICON_COLUMNS) {
        bail!(
            "some single-width character declared a budget other than \
             {ICON_COLUMNS}: the width stopped coming from the font"
        );
    }
    // AND THE GRID'S GAP STILL ABSORBS THE LARGEST. Without this, tightening the
    // horizontal gap would make the 23 paint over their neighbour, and nothing else on
    // screen would say so.
    let step = simple
        .positions
        .windows(2)
        .find(|p| p[0].0 == p[1].0)
        .map(|p| p[1].1 - p[0].1)
        .context("the grid does not have two columns to measure the step")?;
    if largest >= step {
        bail!(
            "the widest ink declares {largest} columns and the grid's step is {step}:\n\
             the double-width characters will paint over their neighbour."
        );
    }
    let budget = largest;
    let normal = ICON_COLUMNS;
    // IT PASTES THE CHARACTER ITSELF to reach it, and not a query by name:
    // which result comes first is the ORDERING's business, and this criterion
    // is about width. Pasting a character from the database matches by identity.
    query(&mut inst, "\u{27F5}")?;
    // AND THE SHEET EXPLAINS WHY THAT CELL LOOKS ODD.
    let f = sheet(&mut inst)?;
    if !keys(&f).iter().any(|(k, _)| k == "unicode-sheet-width") {
        bail!(
            "the sheet of {:?} does not say it is wide (keys: {:?})",
            sheet_value(&f, "unicode-sheet-codepoint"),
            keys(&f).iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()
        );
    }
    inst.event(&Input::Cancel)?;
    println!("wide      budget {budget} against {normal} for the normal one");

    // ---- THE FAMILY PANEL ----
    query(&mut inst, "")?;
    let header = inst.render(COLUMNS, ROWS, heights())?;
    let (icon_line, icon_column) = header
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Icon(n),
                ..
            } if n == "filter" => Some((*line, *column)),
            _ => None,
        })
        .context("the header did not emit the filter icon")?;
    if inst
        .event(&Input::Click {
            column: icon_column,
            line: icon_line,
        })?
        .is_empty()
    {
        bail!("clicking the filter icon did not open the panel");
    }
    let open = inst.render(COLUMNS, ROWS, heights())?;
    let options = panel_of(&open);
    if options.is_empty() {
        bail!("the panel opened with no option at all");
    }
    for example in ["all", "symbol", "letter", "mark"] {
        let key = format!("unicode-family-{example}");
        if !options.iter().any(|(k, ..)| *k == key) {
            bail!("the panel did not offer `{key}` — did the family go away from the database?");
        }
    }
    // EVERY OPTION HAS A KEY IN BOTH LANGUAGES AND FITS THE BUDGET, and the LABEL
    // one reads finds, when typed into the search, the whole family.
    //
    // The last is what ties the two representations of the same fact together: the family's
    // name lives in the `.ftl` (because it gets translated) and becomes a tag in the generator (because
    // it gets searched), and nothing else forces the two to agree.
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, _, _, budget) in &options {
            let Some(text) = table.get(key) else {
                bail!(
                    "the option `{key}` does not exist in {language}.ftl — it would come out as `[{key}]`"
                );
            };
            if text.chars().count() > usize::from(*budget) {
                bail!(
                    "in {language}, `{key}` = {text:?} is {} cells and the panel reserved {budget}",
                    text.chars().count()
                );
            }
            let (prefix, value) = if let Some(f) = key.strip_prefix("unicode-family-") {
                ("familia", f)
            } else if let Some(g) = key.strip_prefix("unicode-group-") {
                ("grupo", g)
            } else if let Some(v) = key.strip_prefix("unicode-tone-") {
                ("tom", v)
            } else {
                continue;
            };
            if value == "all" {
                continue;
            }
            let family_only = query(&mut inst, &format!("{prefix}:{value}"))?.filtered;
            let by_name = query(&mut inst, &text.to_lowercase())?.filtered;
            if by_name < family_only {
                bail!(
                    "in {language}, the panel shows {text:?} and searching for that finds {by_name}\n\
                     characters — `{prefix}:{value}` has {family_only}. The `.ftl` and the generator\n\
                     diverged, and the screen is teaching a word the search does not know."
                );
            }
        }
        println!(
            "panel     {language:8} -> the {} options fit and find",
            options.len()
        );
    }

    // ---- THE EMOJI SUB-FILTERS ARE ALWAYS THERE, AND SEPARATED ----
    //
    // They used to appear only with the `emoji` family selected, and the objection was
    // a good one: under `family:letra`, `group:comida` narrows to ZERO. What it did not
    // foresee is the cost of the opposite — a panel that CHANGES size under the
    // hand, with the options moving between one Tab and the next, and with
    // no way to find out that the sub-filters exist.
    //
    // The answer is not to hide, but to SEPARATE: a rule, a small label, and
    // they stay. This criterion asserts both halves — that they are always there, and
    // that the separation is there with them.
    let subfilters =
        |inst: &mut ronoctua_runtime::Instance, search: &str| -> Result<(usize, usize)> {
            query(inst, search)?;
            if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
                inst.event(&Input::Toggle)?;
            }
            let p = panel_of(&inst.render(COLUMNS, ROWS, heights())?);
            Ok((
                p.iter()
                    .filter(|(k, ..)| k.starts_with("unicode-group-"))
                    .count(),
                p.iter()
                    .filter(|(k, ..)| k.starts_with("unicode-tone-"))
                    .count(),
            ))
        };
    for stance in [
        "familia:emoji",
        "grupo:comida",
        "familia:letra",
        "familia:marca",
        "",
    ] {
        let (groups, tons) = subfilters(&mut inst, stance)?;
        if groups != 9 || tons != 5 {
            bail!(
                "with `{stance}` the panel showed {groups} groups and {tons} tones; \
                 are always 9 and 5"
            );
        }
    }

    // THE SEPARATION: the rule and the section's label, on screen, together with them.
    //
    // Without this criterion, "always visible" would become "mixed in with the rest" — and
    // then the old objection would hold again, because nothing would say what the nine
    // groups apply to.
    query(&mut inst, "familia:letra")?;
    if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        inst.event(&Input::Toggle)?;
    }
    let with_panel = inst.render(COLUMNS, ROWS, heights())?;
    let label = with_panel.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            text: Text::Key(k),
            ..
        } if k == "unicode-panel-emoji" => Some(*line),
        _ => None,
    });
    let Some(label_row) = label else {
        bail!(
            "the panel did not bring the emoji section's label — the subfilters are left with no caption"
        );
    };
    if !with_panel
        .commands
        .iter()
        .any(|c| matches!(c, Command::Rule { line, .. } if *line + 1 == label_row))
    {
        bail!("there is no rule above the section's label — the section does not read as separate");
    }
    // AND THE SUB-FILTERS COME AFTER IT. A label below what it names
    // names something else.
    let first_subfilter = panel_of(&with_panel)
        .into_iter()
        .filter(|(k, ..)| k.starts_with("unicode-group-") || k.starts_with("unicode-tone-"))
        .map(|(_, l, ..)| l)
        .min()
        .context("the subfilters went off the screen")?;
    if first_subfilter <= label_row {
        bail!("the subfilters appear BEFORE the label that names them");
    }

    // THE TONES REALLY DO NARROW, and each narrows a different slice.
    let mut seen = Vec::new();
    for tom in ["light", "medium-light", "medium", "medium-dark", "dark"] {
        let n = query(&mut inst, &format!("tom:{tom}"))?.filtered;
        if n == 0 {
            bail!("`tom:{tom}` finds nothing — the tone did not reach the database");
        }
        seen.push(n);
    }
    let soma: usize = seen.iter().sum();
    let so_emoji = query(&mut inst, "familia:emoji")?.filtered;
    if soma >= so_emoji {
        bail!("the five tones add up to {soma} of {so_emoji} emoji — they are not cutting");
    }
    // AND A FREE TERM COEXISTS with the tone, as with the other enumeration
    // filters: `tom:dark mao` is the actual gesture of whoever is looking for a hand.
    let so_tom = query(&mut inst, "tom:escuro")?.filtered;
    let with_term = query(&mut inst, "tom:escuro mao")?.filtered;
    if with_term == 0 || with_term >= so_tom {
        bail!("`tom:escuro mao` gave back {with_term} of {so_tom} — the free term did not cut");
    }
    inst.event(&Input::Cancel)?;
    println!(
        "subfilters 9 groups and 5 tones in every stance, separated by a rule and a label\n\
         tons      {seen:?} de {so_emoji} emoji · `tom:escuro mao` -> {with_term}"
    );

    // ---- WHERE THE SELECTION IS, IN A GRID THAT DOES NOT OBEY COLOUR ----
    //
    // The token's colour is no use for emoji: it is INSIDE the glyph (a CBDT
    // bitmap), and `state-accent` does not change a pixel of them. In 3,944 of the 11,614
    // cells the selection was invisible — and precisely in the ones the buffer OPENS
    // showing. The marker is an icon, on the row below the focused cell.
    let marker = |list: &ronoctua_protocol::DisplayList| -> Option<(u16, u16)> {
        list.commands.iter().find_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Icon(i),
                ..
            } if i == "selection" => Some((*line, *column)),
            _ => None,
        })
    };
    query(&mut inst, "familia:emoji")?;
    let before = marker(&inst.render(COLUMNS, ROWS, heights())?)
        .context("the grid did not mark the selection — on emoji it would be invisible")?;
    inst.event(&Input::Right)?;
    let after = marker(&inst.render(COLUMNS, ROWS, heights())?)
        .context("the marker went away when the cursor moved")?;
    if before == after {
        bail!("the marker did not move with the cursor: it stayed at {before:?}");
    }
    if before.0 != after.0 {
        bail!("moving sideways changed the marker's LINE: {before:?} -> {after:?}");
    }
    inst.event(&Input::Left)?;
    println!("marker    {before:?} -> {after:?} on moving one cell");

    // ---- `Tab` TOGGLES, AND DOES NOT GO INTO THE SEARCH ----
    //
    // The other driver's approach (looking for the invitation on screen) does NOT work here:
    // this buffer opens with text in the field and never shows the invitation. The
    // assertion has to be about the string, byte by byte.
    // IT CLOSES THE PANEL FIRST. It has been open since the click on the icon, and a
    // `Tab` over an open panel CLOSES it — the test would measure the opposite of what it
    // wants to measure.
    if !panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        inst.event(&Input::Cancel)?;
    }
    query(&mut inst, INITIAL_FILTER)?;
    let before = screen(&mut inst)?.field.unwrap_or_default();
    inst.event(&Input::Toggle)?;
    if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        bail!("`Tab` did not open the panel");
    }
    inst.event(&Input::Toggle)?;
    let screen_after = inst.render(COLUMNS, ROWS, heights())?;
    if !panel_of(&screen_after).is_empty() {
        bail!("the second `Tab` did not close the panel");
    }
    let after = read(&screen_after)?.field.unwrap_or_default();
    if before != after {
        bail!(
            "the field was {before:?} and after two `Tab`s it is {after:?}: the key went\n\
             into the search as a character, and a tab in the field is invisible."
        );
    }
    println!("tab       opens, closes, and does not touch the field");

    // ---- THE CLICK LANDS ON WHAT WAS DRAWN THERE ----
    query(&mut inst, "familia:letra")?;
    let grid = screen(&mut inst)?;
    const TARGET: usize = 3;
    let (line, column) = *grid
        .positions
        .get(TARGET)
        .context("the grid did not draw enough cells")?;
    let drawn = grid.drawings[TARGET].clone();
    if inst.event(&Input::Click { column, line })?.is_empty() {
        bail!("the click at ({column},{line}) asked for no redraw");
    }
    // THE FOCUS IS READ FROM THE SHEET, and not from the drawing: for a mark the drawing starts
    // at the circle, and comparing drawings would give false.
    let f = sheet(&mut inst)?;
    let cp = sheet_value(&f, "unicode-sheet-codepoint").context("a sheet with no codepoint")?;
    let expected = drawn
        .chars()
        .next_back()
        .map(|c| format!("U+{:04X}", c as u32))
        .unwrap_or_default();
    if cp != expected {
        bail!("the click at ({column},{line}) focused {cp}, and {expected} was drawn there");
    }
    inst.event(&Input::Cancel)?;
    println!("click    ({column},{line}) -> {cp}");

    // ---- THE SHEET CARRIES THE VALUES, AND THE COPY REPORTS BACK ----
    query(&mut inst, "FOR ALL")?;
    let f = sheet(&mut inst)?;
    let values: Vec<String> = f
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Data(s),
                ..
            } => Some(s.clone()),
            _ => None,
        })
        .collect();
    for expected in ["U+2200", "8704", "E2 88 80", "\\u{2200}", "&#x2200;"] {
        if !values.iter().any(|v| v == expected) {
            bail!(
                "the sheet of `FOR ALL` does not show {expected:?}. A list of labels with no value\n\
                 is a list of promises: {values:?}"
            );
        }
    }
    // `Down` + `Enter` copies the SECOND option, and not the first.
    inst.event(&Input::Down)?;
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copying did not ask for Copy")?;
    if copied != "FOR ALL" {
        bail!("down+Enter copied {copied:?}; the second option is the NAME");
    }
    // THE VERDICT APPEARS, and goes away on the next key.
    inst.event(&Input::Copied(true))?;
    if !keys(&inst.render(COLUMNS, ROWS, heights())?)
        .iter()
        .any(|(k, _)| k == "unicode-sheet-copied")
    {
        bail!("after copying, the sheet does not show the confirmation");
    }
    inst.event(&Input::Down)?;
    if keys(&inst.render(COLUMNS, ROWS, heights())?)
        .iter()
        .any(|(k, _)| k == "unicode-sheet-copied")
    {
        bail!("the copy notice stayed on screen after the next key");
    }
    // EVERY SHEET LABEL FITS THE BUDGET, in both languages.
    let keys = keys(&inst.render(COLUMNS, ROWS, heights())?);
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, budget) in &keys {
            let Some(text) = table.get(key) else {
                bail!("the sheet's key `{key}` does not exist in {language}.ftl");
            };
            if text.chars().count() > usize::from(*budget) {
                bail!(
                    "in {language}, `{key}` = {text:?} is {} cells and the buffer reserved {budget}",
                    text.chars().count()
                );
            }
        }
    }
    inst.event(&Input::Cancel)?;
    println!("sheet     values, copy with a verdict, and the labels fit");

    // ---- PASTING A CHARACTER FINDS THE CHARACTER, EVEN WITH THE FILTER ACTIVE ----
    //
    // THE DEFECT THIS CATCHES, and only this buffer has it: it OPENS filtered on
    // symbols. Pasting a letter would return "no results" — because of a
    // filter whoever pasted did not type. The question is "what character is this?",
    // and the answer must not depend on the posture the buffer happened to be in.
    query(&mut inst, INITIAL_FILTER)?;
    inst.event(&Input::Paste("α".to_owned()))?;
    let pasted = screen(&mut inst)?;
    if pasted.filtered == 0 {
        bail!("pasting a LETTER with the symbol filter active found nothing");
    }
    let f = sheet(&mut inst)?;
    let cp = sheet_value(&f, "unicode-sheet-codepoint").context("a sheet with no codepoint")?;
    if cp != "U+03B1" {
        bail!("pasting `α` found {cp}");
    }
    inst.event(&Input::Cancel)?;
    println!("paste     `α` with `simbolos:` active -> {cp}");

    // ---- AND THE FIELD IS IN THE PERSON'S LANGUAGE ----
    //
    // THE DEFECT THIS CATCHES, and it was on a real screen: the field is the
    // one text this buffer both HOLDS and SHOWS, so it can be neither a key
    // (the buffer could not read it back) nor a literal (it opened
    // `familia:emoji` on an en-GB session). It is a phrase, and this is the
    // round trip that proves the whole mechanism:
    //
    //   * seeded pt-BR, the buffer OPENS in `familia:emoji`;
    //   * what it wrote, it can READ BACK — `Esc` drops the free term and
    //     leaves the prefix, still in Portuguese, and the count does not move.
    //
    // A SECOND INSTANCE, because phrases arrive once and before the first
    // frame — which is exactly the contract, and re-seeding a running one
    // would be testing something the host never does.
    let mut other = rt.open(&component)?;
    let d_of = other.init()?;
    ronoctua_runtime::driven::hand_phrases(&mut other, &d_of, &i18n, "pt-BR")?;
    let opened = screen(&mut other)?;
    let in_portuguese = opened
        .field
        .clone()
        .context("the pt-BR field emitted nothing on opening")?;
    if in_portuguese != "familia:emoji" {
        bail!("seeded pt-BR the buffer opened with `{in_portuguese}`, not `familia:emoji`");
    }
    if opened.filtered != initial.filtered {
        bail!(
            "the pt-BR field says `{in_portuguese}` and filters {} — the en-GB one filters {}.\n\
             The word changed and the FILTER changed with it: the buffer wrote a\n\
             prefix it cannot read back.",
            opened.filtered,
            initial.filtered
        );
    }
    other.event(&Input::Paste(" seta".to_owned()))?;
    other.event(&Input::Cancel)?;
    let after = screen(&mut other)?;
    let left = after.field.unwrap_or_default();
    if left != "familia:emoji" {
        bail!("`Esc` left `{left}` and not the pt-BR prefix `familia:emoji`");
    }
    println!("language  pt-BR -> `{in_portuguese}`, and Esc gives it back");

    println!(
        "OK: the database has {TOTAL}, opens filtered and leaves the filter, the filters narrow,"
    );
    println!("    the marks show over the circle and copy clean, the wide ones are not");
    println!("    elided, the panel ties label to search, and the sheet copies with a notice");
    Ok(())
}
